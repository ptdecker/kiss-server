# Project Research Summary

**Project:** ptodd — Pure-Rust HTTP/1.1 Static File Server
**Domain:** Systems programming / HTTP server (no external dependencies)
**Researched:** 2026-02-28
**Confidence:** HIGH

## Executive Summary

ptodd is a learning-oriented, pure-Rust HTTP/1.1 static file server built on `std` only (plus `log` v0.4.20). The project already has working TCP acceptance, a thread pool, request parsing, and a custom `DateTime` type — but the existing code has critical bugs that make it unsafe for any real use: it panics on malformed input, reads files incorrectly (string I/O for binary), has hardcoded file paths, and has no defense against path traversal. The correct approach is a structured refactor in dependency order: stabilize the broken foundations first, then layer the new `Response`/`Handler`/`Router` architecture on top, and finally implement `StaticFileHandler` with its full security surface.

The recommended architecture is a small, explicit component graph: a `Response` struct owns serialization, a `Handler` trait provides the single extension point for both static files and future REST endpoints, a `Router` dispatches based on method and path, and `StaticFileHandler` implements the handler trait with full path traversal protection. This design is idiomatic for a no-async, thread-pool server and is directly extensible to REST without architectural changes. All required APIs exist in `std`; no new dependencies are needed.

The most significant risks are security (path traversal via `..` and `PathBuf::join()` absolute-path replacement) and stability (mutex poison cascade from worker thread panics). Both classes of risk have clear, well-understood mitigations in pure `std`. The correct order — fix bugs before adding features, add path security as part of `StaticFileHandler`'s initial implementation rather than as a later polish step — is the only safe order. Skipping the bug fixes first creates a foundation where new features inherit existing failure modes.

## Key Findings

### Recommended Stack

The stack is locked by the project constraints: Rust 2021 edition, `log` v0.4.20, and zero new dependencies. All needed APIs are present in `std` and have been stable since Rust 1.56. The key `std` modules are `std::fs` (file reading with `fs::read()` for binary-safe I/O), `std::path` (`Path::canonicalize()` and `Path::starts_with()` for traversal prevention), `std::io` (`BufReader`, `BufWriter`, `io::copy` for streaming), and `std::net` (`TcpStream::set_read_timeout` for DoS prevention). The existing `ThreadPool`, `DateTime`, and `Url` types are in-tree and require targeted fixes rather than replacement.

**Core technologies:**
- `std::fs::File` + `std::io::copy`: binary-safe file streaming — replaces `fs::read_to_string` which corrupts binary files
- `std::path::Path::canonicalize()` + `starts_with()`: path traversal prevention — the only authoritative approach; manual `..` stripping is insufficient
- `std::io::BufWriter<TcpStream>`: response buffering — batches header and body writes to avoid per-syscall overhead
- `std::net::TcpStream::set_read_timeout()`: DoS mitigation — unblocks worker threads from stalled clients
- In-tree `Url::pct_decode()`: percent-decode before filesystem resolution — utility already exists; needs wiring into handler pipeline

### Expected Features

The 14-item MVP feature set is fully defined and internally consistent. All items are implementable in `std` with no ambiguity about approach.

**Must have (table stakes for v1):**
- Configurable static root directory via CLI argument — fixes the hardcoded-path crash bug
- Binary-safe file reading (`fs::read()`, `Vec<u8>` body) — fixes silent binary file corruption
- MIME type detection from file extension — required for browser to interpret files correctly
- Path traversal prevention (percent-decode, canonicalize, prefix check, trim leading `/`) — critical security
- Request header size limit (100 headers / 8KB per line) — blocks header-exhaustion DoS
- Clean 400/404/405/500 error responses — replaces current panic-on-bad-input behavior
- `Content-Type`, `Content-Length`, `Date`, `Connection: close` headers on all responses — HTTP/1.1 compliance
- HEAD method support — RFC 9110 requirement for static resources
- Query string stripping before file lookup — prevents spurious 404s on `?v=2` cache-busting URLs
- Remove the `/sleep` test endpoint — eliminates known DoS vector

**Should have (deferred to v2):**
- `If-Modified-Since` / `Last-Modified` conditional GET — enables browser caching; medium complexity
- `index.html` implicit serving for directory requests — useful UX; extension point is already in `StaticFileHandler`
- Range requests — needed for video streaming; high complexity via `std::io::Seek`

**Defer (v2+):**
- `ETag` / `If-None-Match` — robust caching; depends on stable hash or mtime
- Compression (gzip/br) — requires a compression library; conflicts with no-deps constraint
- Keep-alive / persistent connections — requires architecture change (multi-request per connection)

### Architecture Approach

The architecture is a linear dependency chain that must be built in strict order. `Response` is the foundation — every handler produces one. The `Handler` trait sits above it and provides the extension interface. The `Router` sits above that, dispatching `&Request` to the right handler. `StaticFileHandler` is the first concrete handler. `handle_connection()` is thinned to: parse request, dispatch to router, write response. The existing `Request`, `RequestMethod`, `Url`, and `ThreadPool` types plug into this without structural changes, though `Url` needs two new public methods (`path()` and `decoded_path()`).

**Major components:**
1. `src/server/response.rs` — `Response` struct: owns status, headers (`Vec<(String, String)>`), body (`Vec<u8>`); `to_bytes()` serializes with CRLF; `set_body()` auto-sets `Content-Length`
2. `src/server/handler.rs` — `Handler` trait (`Send + Sync`, `fn handle(&self, &Request) -> Result<Response>`); `NotFoundHandler` as router fallback
3. `src/server/router.rs` — `Router` with `Vec<Route>` + fallback handler; matches on `(RequestMethod, exact path string)`; wrapped in `Arc<Router>` for cross-thread sharing
4. `src/server/static_files.rs` — `StaticFileHandler`: stores `PathBuf` root (canonicalized at construction); decodes path, strips leading `/`, joins, canonicalizes, checks prefix, checks `is_dir()`, reads with `fs::read()`
5. `src/server/mime.rs` — `mime_for_extension()`: pure `match` on `Path::extension()`, returning `&'static str`
6. `src/url/mod.rs` (extension) — add `path()` (raw path, no query) and `decoded_path()` (pct-decoded); router uses raw, handler uses decoded

### Critical Pitfalls

1. **Path traversal via `..` after percent-decode** — three independent defenses are required simultaneously: (a) reject decoded path segments equal to `..`, (b) strip leading `/` before `PathBuf::join()` to prevent root replacement, (c) `canonicalize()` + `starts_with(root)` as the definitive check. Any single defense alone is insufficient.

2. **`fs::canonicalize()` fails on non-existent paths** — `canonicalize()` calls `realpath(3)` which requires the path to exist. Handle the `NotFound` error as a 404, but only after the component-level `..` rejection has already run. Never skip the traversal check in the error path.

3. **Mutex poison cascade kills all workers** — if one worker panics while holding the job queue lock, the mutex becomes poisoned. Any subsequent `.expect()` or `.unwrap()` on `lock()` panics the next worker. All workers die silently; the server accepts connections but handles none. Fix: handle `PoisonError` explicitly by calling `poisoned.into_inner()` and continuing.

4. **Unbounded header reads enable DoS with 4 connections** — `BufReader::lines()` with no limit blocks indefinitely on a client that never sends a blank line. Fix: `reader.lines().take(MAX_HEADERS)` plus a per-line size limit. Return 400 when the limit is hit.

5. **`Response::to_bytes()` using `\n` instead of `\r\n`** — HTTP/1.1 requires CRLF line terminators (RFC 9112 §2.2). This works with lenient browsers but fails strict clients and test suites. Fix: define `const CRLF: &[u8] = b"\r\n"` and unit-test the raw bytes of `to_bytes()` output.

## Implications for Roadmap

Based on research, the architecture defines a strict build order. Attempting to implement features before fixing the foundation means new code inherits existing crashes and security holes. The suggested phase structure maps directly to the dependency order established in ARCHITECTURE.md, cross-referenced against the MVP feature list in FEATURES.md and the phase warnings in PITFALLS.md.

### Phase 1: Foundation Bug Fixes

**Rationale:** The existing code has active crashes (unwrap on malformed input, read_to_string on binary files) and a silent DoS vector (unbounded header reads, mutex poison cascade). Any new code built on this foundation inherits these failures. Fixing the foundation first means subsequent phases build on stable ground.

**Delivers:** A server that does not crash on bad input, handles errors gracefully, and resists the simplest DoS attacks. No new user-visible features, but the server becomes safe to test against real clients.

**Addresses (from FEATURES.md MVP list):**
- Remove `/sleep` test endpoint
- 400 Bad Request on malformed requests (fix crash bug)
- Request header size limit
- Return 404 for missing files (clean error, not crash)

**Avoids (from PITFALLS.md):**
- Pitfall 3: Mutex poison cascade (fix `.expect()` in worker loop and `Drop`)
- Pitfall 4: Unbounded header read DoS (add `take(MAX_HEADERS)`)
- Pitfall 5: Non-UTF-8 panic (propagate `?` on `lines()`)
- Pitfall 13: `unsafe unwrap_unchecked` in `DateTime`
- Pitfall 14: Double-panic in `ThreadPool::drop()`
- Pitfall 16: DateTime boundary bugs near year/month edges

**Research flag:** Skip — these are all well-defined bugs with known fixes from the existing CONCERNS.md analysis.

### Phase 2: Response Struct

**Rationale:** Every subsequent component produces a `Response`. Building this before the handler trait or router means those components have something concrete to build against and test with. This phase also delivers all mandatory HTTP/1.1 headers in one place so they can never be forgotten on individual response paths.

**Delivers:** `src/server/response.rs` with `Response` struct, `to_bytes()` (CRLF-correct), `set_body()` (auto-sets `Content-Length`), and a `Response::new()` that automatically adds `Date` and `Server` headers.

**Addresses (from FEATURES.md MVP list):**
- `Content-Type`, `Content-Length`, `Date`, `Connection: close` on all responses
- 400/404/500 response bodies

**Avoids (from PITFALLS.md):**
- Pitfall 6: Content-Length from bytes not string chars (build into `set_body()`)
- Pitfall 8: Date header missing (add in `Response::new()`)
- Pitfall 11: `\r\n` vs `\n` (test `to_bytes()` at byte level)

**Research flag:** Skip — `Response` struct pattern for no-async HTTP servers is well-established; std APIs are stable and documented in STACK.md.

### Phase 3: Handler Trait and Router

**Rationale:** The `Handler` trait establishes the interface that `StaticFileHandler` and all future REST handlers implement. The `Router` wires existing request parsing to handler dispatch. Building both before `StaticFileHandler` means the server can be end-to-end tested with a `NotFoundHandler` fallback before any file serving exists.

**Delivers:** `src/server/handler.rs` (`Handler` trait + `NotFoundHandler`), `src/server/router.rs` (`Router` with `Arc` sharing), and integration into `handle_connection()` replacing the hardcoded `match` block.

**Uses (from STACK.md):** `Arc<Router>` for thread-safe sharing; `Box<dyn Handler>` for dynamic dispatch without generics complexity

**Implements (from ARCHITECTURE.md):** Handler trait and Router components; `Arc<Router>` field on `Server`

**Research flag:** Skip — the `Box<dyn Handler + Send + Sync>` pattern for thread-safe dynamic dispatch is standard Rust; no novel APIs involved.

### Phase 4: URL Path and Decode Methods

**Rationale:** `StaticFileHandler` requires `Url::path()` (raw path for routing) and `Url::decoded_path()` (pct-decoded for filesystem resolution). These must be defined before `StaticFileHandler` can be implemented correctly. The decode contract — router uses raw path, handler decodes immediately before filesystem access — must be established here to prevent Pitfall 12.

**Delivers:** `path()` and `decoded_path()` on the existing `Url` struct; removal of `#![allow(unused)]` from the url module.

**Avoids (from PITFALLS.md):**
- Pitfall 12: Decode in handler not router (define the contract explicitly in method documentation)

**Research flag:** Skip — the existing `pct_decode()` utility already exists in the codebase; this is wiring, not new logic.

### Phase 5: StaticFileHandler and MIME Detection

**Rationale:** With `Response`, `Handler`, `Router`, and `Url::decoded_path()` all in place, `StaticFileHandler` can be built on a complete foundation. Path traversal protection must be in the initial implementation — it is not a follow-up hardening step. All three traversal defenses (component rejection, leading-slash strip, canonicalize+prefix check) must be present together.

**Delivers:** `src/server/static_files.rs` (`StaticFileHandler` with full path safety), `src/server/mime.rs` (extension-based MIME table), and a working static file server accepting a configurable root directory via CLI argument.

**Addresses (from FEATURES.md MVP list):**
- Configurable static root directory
- Binary-safe file reading (`fs::read()`)
- MIME type detection (html, css, js, wasm, images, fonts, and fallback)
- Path traversal prevention (all three defenses)
- 404 for missing files and for directory requests
- Query string stripping before file lookup

**Avoids (from PITFALLS.md):**
- Pitfall 1: Path traversal via `..` (component rejection + canonicalize + starts_with)
- Pitfall 2: canonicalize on missing file (two-phase: component check, then canonicalize, handle NotFound)
- Pitfall 7: charset=utf-8 missing from text/* MIME types
- Pitfall 9: Serving directories as files (fs::metadata().is_dir() check before read)
- Pitfall 10: PathBuf::join() discards root on absolute path (trim_start_matches('/') before join)
- Pitfall 15: application/javascript deprecated (use text/javascript)

**Research flag:** Recommend deeper review during implementation — path traversal has multiple interacting defenses that must all be present simultaneously. A security-focused code review or test suite against the three attack vectors (encoded `..`, absolute path, symlink escape) is advisable before considering this phase done.

### Phase 6: HTTP/1.1 Method Compliance

**Rationale:** HEAD method support (same headers, no body) and 405 Method Not Allowed for POST/PUT/DELETE are required by RFC 9110 but depend on the router and response infrastructure from Phases 2-3. Grouping these in a dedicated phase after the core file serving works means compliance can be tested against a real browser or `curl`.

**Delivers:** HEAD method routing to `StaticFileHandler` with body suppressed; 405 responses for non-GET/HEAD methods on static paths; `Host` header validation returning 400 if absent.

**Addresses (from FEATURES.md MVP list):**
- HEAD method support
- 405 Method Not Allowed for POST/PUT/DELETE
- Host header validation (HTTP/1.1 RFC 9112 §3.2)

**Research flag:** Skip — HEAD method behavior (same headers, empty body) and 405 semantics are precisely specified in RFC 9110; no ambiguity in implementation approach.

### Phase Ordering Rationale

- Phases 1-2 must come before everything else because broken error handling and missing `Response` infrastructure make all subsequent work unstable.
- Phase 3 (Handler/Router) must precede Phase 5 (StaticFileHandler) because the handler trait is the interface the static file handler implements.
- Phase 4 (URL methods) is independent of Phase 3 but must precede Phase 5.
- Phase 5 is the central deliverable — the point at which the server actually serves files correctly and safely.
- Phase 6 adds compliance polish on top of a working system; it does not gate Phase 5.
- This ordering means each phase produces a working, testable server state. No phase leaves the server in a broken intermediate state.

### Research Flags

Phases needing deeper implementation-time scrutiny:
- **Phase 5 (StaticFileHandler):** Path traversal has three required defenses that interact. Recommend testing each attack vector (encoded `..`, `PathBuf::join()` absolute path replacement, symlink escape) explicitly before closing the phase.

Phases with well-established patterns (no additional research needed):
- **Phase 1:** All bugs are identified in CONCERNS.md with known fixes.
- **Phase 2:** `Response` struct pattern is standard for no-async Rust HTTP servers.
- **Phase 3:** `Box<dyn Handler + Send + Sync>` and `Arc<Router>` are established Rust patterns.
- **Phase 4:** Wiring existing decode utility into struct API — no novel logic.
- **Phase 6:** HEAD and 405 behavior is precisely specified in RFC 9110.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All APIs are from stable `std`; present since Rust 1.56; no deprecations in scope |
| Features | HIGH | MVP feature list drawn from RFC 9110/9112 requirements and direct codebase analysis |
| Architecture | HIGH | Component boundaries, build order, and integration points are fully specified with concrete code shapes |
| Pitfalls | HIGH | All pitfalls grounded in Rust stdlib documented behavior, RFC specifications, and verified CONCERNS.md analysis |

**Overall confidence:** HIGH

### Gaps to Address

- **`DateTime` HTTP format output:** The existing `DateTime` struct needs a new `Display` format for the `Date` header (RFC 7231 IMF-fixdate: `Thu, 01 Jan 1970 00:00:00 GMT`). Weekday derivation formula is provided in STACK.md (`epoch_days % 7`, offset from Thursday). This is low-risk but must be verified with test cases at month and year boundaries (Pitfall 16 is a known issue here).
- **CLI argument parsing scope:** FEATURES.md specifies that the static root and server address come from CLI args (`std::env::args()`). The parsing strategy (positional vs. flag-based) is not specified in research; this is a small design decision needed early in Phase 1 or before Phase 5.
- **Signal handling:** True SIGTERM handling requires either `unsafe` FFI or an external crate, both of which are out of scope. SIGINT (Ctrl+C) causes `TcpListener::incoming()` to error, which breaks the accept loop and runs `ThreadPool::drop()`. This is documented as a known limitation, not a bug to fix.

## Sources

### Primary (HIGH confidence)
- RFC 9110 (HTTP Semantics) — https://www.rfc-editor.org/rfc/rfc9110 — mandatory headers, status codes, method semantics
- RFC 9112 (HTTP/1.1 Message Syntax) — https://www.rfc-editor.org/rfc/rfc9112 — CRLF, Content-Length, Host header requirement
- RFC 3986 (URI Generic Syntax) — https://www.rfc-editor.org/rfc/rfc3986 — percent-encoding, path normalization
- RFC 9239 (Updates to ECMAScript Media Types) — text/javascript IANA registration
- Rust std library documentation (stable, since 1.56) — `std::fs`, `std::path`, `std::io`, `std::net`, `std::sync`
- CONCERNS.md — codebase audit identifying specific bugs and security gaps (direct code analysis)

### Secondary (MEDIUM confidence)
- 8KB header size limit — widely adopted industry convention; not normative in RFC 9112 but standard practice
- `Connection: close` as DoS prevention for thread-pool servers without keep-alive — common architectural decision

---
*Research completed: 2026-02-28*
*Ready for roadmap: yes*
