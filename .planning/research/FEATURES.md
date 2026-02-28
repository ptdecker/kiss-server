# Feature Landscape

**Domain:** HTTP/1.1 Static File Server (Rust, no external dependencies)
**Researched:** 2026-02-28
**Confidence:** HIGH for table stakes (RFC 9110/9112 requirements are stable, well-established)

---

## Note on Research Constraints

Web search and WebFetch were unavailable during this session. RFC 9110 (HTTP Semantics) and RFC 9112
(HTTP/1.1) are mature, stable specifications. The table stakes requirements below are drawn from
those specs using HIGH confidence training knowledge. All RFC citations are verifiable at
rfc-editor.org.

---

## Table Stakes

Features the server must have, or it is broken, unsafe, or non-compliant. Missing any of these
means the server cannot be called "correct."

### Response Headers

| Feature | Why Required | Complexity | RFC Reference |
|---------|--------------|------------|---------------|
| `Content-Length` header on all responses | Without it, HTTP/1.1 clients cannot determine message boundaries; persistent connections break | Low | RFC 9110 §8.6 |
| `Content-Type` header on all non-empty responses | Without it, browsers guess type (MIME sniffing); security issue and behavior varies per UA | Low | RFC 9110 §8.3 |
| `Date` header on all responses | Required for cache correctness; RFC 9110 §6.6.1 says servers SHOULD send it; most clients expect it | Low | RFC 9110 §6.6.1 |
| Correct HTTP version in status line | Responses must say `HTTP/1.1`, not HTTP/1.0 | Trivial | RFC 9112 §4 |
| Standard status codes: 200, 404, 400, 500 | Clients depend on status codes to interpret responses; incorrect codes cause silent failures | Low | RFC 9110 §15 |

### MIME Type Detection

| Feature | Why Required | Complexity | Notes |
|---------|--------------|------------|-------|
| `.html` → `text/html; charset=utf-8` | Without charset, browsers may misrender non-ASCII content | Low | Must include charset parameter |
| `.css` → `text/css` | Browsers refuse to apply CSS with wrong MIME type (strict MIME checking) | Low | — |
| `.js` → `text/javascript` | Browsers refuse to execute JS with wrong MIME type | Low | `application/javascript` is deprecated; use `text/javascript` |
| `.wasm` → `application/wasm` | WASM modules require exact MIME type to instantiate | Low | PROJECT.md lists WASM as a target file type |
| Unknown extension → `application/octet-stream` | Forces download behavior rather than guessing; safe default | Low | — |
| Extension lookup is case-insensitive | `.HTML` and `.html` must resolve to the same MIME type | Low | — |

### Path Safety

| Feature | Why Required | Complexity | Notes |
|---------|--------------|------------|-------|
| Path traversal prevention | Without it, `GET /../../../etc/passwd` serves arbitrary files from the filesystem | Medium | CONCERNS.md explicitly flags this; critical security requirement |
| Resolve canonical path before serving | Symlinks outside root can escape containment even if the literal path looks safe | Medium | `std::fs::canonicalize()` then verify prefix |
| Reject null bytes in paths | Null byte injection can truncate filenames on some OS calls | Low | `%00` in percent-encoded path |
| Percent-decode path before safety check | Must decode first, then check for traversal; checking encoded path misses `%2E%2E%2F` attacks | Medium | The `pct_decode` utility already exists in `src/url/mod.rs` |
| Validate path is under configured static root | Absolute final check after normalization: `canonical_path.starts_with(root)` | Low | — |

### Request Handling

| Feature | Why Required | Complexity | Notes |
|---------|--------------|------------|-------|
| Maximum request header size limit | Without it, an attacker sends infinite headers and exhausts memory | Low | CONCERNS.md flags this; 8KB is a common limit |
| Respond 400 Bad Request to malformed requests | Currently panics on malformed input (CONCERNS.md line 41-45); must return 400 instead | Low | RFC 9110 §15.5.1 |
| Handle GET and HEAD methods for static files | HEAD must return same headers as GET but no body; required for cache validation and link checking | Medium | RFC 9110 §9.3.1–9.3.2 |
| Return 405 Method Not Allowed for POST/PUT/DELETE to static paths | Static files are read-only; other methods must be rejected with proper status | Low | RFC 9110 §15.5.6 |
| `Host` header required in HTTP/1.1 requests | RFC 9112 requires HTTP/1.1 clients to send Host; server must return 400 if absent | Low | RFC 9112 §3.2 |

### File Serving

| Feature | Why Required | Complexity | Notes |
|---------|--------------|------------|-------|
| Configurable static root directory | Hardcoded paths are a known bug (CONCERNS.md line 33-37); must be a CLI arg or env var | Low | PROJECT.md requires this |
| Return 404 for missing files | Server currently crashes on missing files (read error propagation problem); must be a clean 404 | Low | — |
| Return 500 for filesystem read errors (permissions, I/O) | Distinct from 404; do not conflate "file not found" with "cannot read file" | Low | — |
| Serve file contents correctly (binary-safe) | `fs::read_to_string` corrupts binary files (images, WASM, fonts); must use `fs::read` returning bytes | Medium | Current code uses `read_to_string` which is wrong for non-UTF-8 files |
| `Content-Length` must reflect actual byte length | For binary files, byte length differs from char count; must use `buf.len()` not string length | Low | Follows from binary-safe file reading |
| Strip query strings before path lookup | `GET /file.html?v=2` should serve `file.html`, not fail with file-not-found | Low | RFC 3986 §3.4 — query component is separate from path |

### Error Responses

| Feature | Why Required | Complexity | Notes |
|---------|--------------|------------|-------|
| 400 Bad Request with body | Currently the server crashes on parse errors; minimum: return status + `Content-Length: 0` | Low | — |
| 404 Not Found with body | Currently uses a hardcoded file (CONCERNS.md line 33); needs a generated or embedded fallback | Low | Must not crash if `404.html` is missing |
| 500 Internal Server Error | For unexpected failures (permissions, I/O errors that are not ENOENT) | Low | — |
| Error responses must include `Content-Length` | Same rule as success responses; omitting it breaks HTTP/1.1 framing | Trivial | — |

### Connection Handling

| Feature | Why Required | Complexity | Notes |
|---------|--------------|------------|-------|
| `Connection: close` header (or equivalent) | This server does not support HTTP/1.1 keep-alive (blocking I/O thread pool); telling clients prevents them from waiting forever | Low | RFC 9110 §7.6.1 — clients may assume persistent connections by default in HTTP/1.1 |
| Close TCP connection after response | Without explicit close, HTTP/1.1 clients may hang waiting for more data | Low | Follows from Connection: close |

---

## Differentiators

Features that add value but are not required for correctness or safety. These are v2+ concerns.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| `If-Modified-Since` / `Last-Modified` conditional GET | Enables browser caching; reduces bandwidth; requires accurate file mtime | Medium | RFC 9110 §8.8.2, §13.1.3 |
| `ETag` / `If-None-Match` conditional GET | More robust caching than Last-Modified alone | High | Requires stable hash or mtime-based ETag generation |
| `Cache-Control` headers | Fine-grained client caching control (immutable, max-age) | Low | Nice to have; not required for correctness |
| Range requests (`Range` / `Content-Range`) | Enables resume-capable downloads; needed for video streaming | High | RFC 9110 §14 |
| `gzip` / `br` Content-Encoding | Reduces transfer size; check `Accept-Encoding` request header | High | Cannot use stdlib alone — requires compression library (conflicts with no-deps constraint) |
| Directory listing | Show `index.html` of a directory or generate a listing | Medium | PROJECT.md explicitly out-of-scope |
| `index.html` implicit serving | `GET /` serves `/index.html` if it exists | Low | Minimal complexity; useful UX improvement |
| Configurable `404.html` | Custom 404 page from the static root | Low | Removes hardcoded file path dependency |
| `Server` header | Identify the server software in responses | Trivial | Conventional; not required |
| `OPTIONS *` response | Advertise supported methods | Low | RFC 9110 §9.3.7 |
| Request timeout | Close connections that stall mid-request | Medium | `TcpStream::set_read_timeout` — prevents thread pool exhaustion from slow clients |

---

## Anti-Features

Things to deliberately NOT build in v1. Building them would add complexity, risk, or maintenance
burden without serving the milestone goal of "correct and safe static file server."

| Anti-Feature | Why Avoid | What to Do Instead |
|--------------|-----------|-------------------|
| Keep-alive / persistent connections | Thread-per-connection model makes persistent connections expensive; a stalled client holds a thread forever | Send `Connection: close`; close after each response |
| HTTP pipelining | Pipelining over persistent connections; not worth implementing without keep-alive | Not applicable |
| Async I/O (tokio/async-std) | PROJECT.md explicitly excludes this; adds external dependency | Synchronous blocking I/O with thread pool is correct for this scope |
| TLS/HTTPS | PROJECT.md explicitly out of scope | Document that TLS termination should happen at a reverse proxy (nginx, caddy) |
| Authentication | PROJECT.md describes an intentionally open service | Document intentional choice |
| Directory listing | PROJECT.md out of scope; adds complexity and security surface | Return 404 for directory requests |
| Compression (gzip/br) | Requires compression library; conflicts with no-external-deps constraint | Rely on reverse proxy for compression |
| Virtual hosting (multiple roots per `Host`) | Over-engineering for a learning project | Single root directory |
| Dynamic REST handlers | Router should be extensible but REST handlers are not built yet | Design router interface for extensibility; do not implement REST handling |
| Metrics / observability endpoints | Out of scope per PROJECT.md | Structured logging is sufficient for v1 |
| Config file parsing | PROJECT.md says CLI args or env vars is sufficient | Use `std::env::args()` and `std::env::var()` |
| The `/sleep` test endpoint | CONCERNS.md flags this as a DoS vector (5s block holds a thread) | Remove before any real use; it is already identified as test scaffolding |

---

## Feature Dependencies

```
Configurable static root
  └─> Static file serving (cannot serve without a root)
        └─> MIME type detection (needed per-file)
        └─> Binary-safe file reading (fs::read, not read_to_string)
              └─> Content-Length from byte buffer length

Path safety (canonicalize + prefix check)
  └─> Percent-decode before checking (pct_decode utility already exists)
        └─> Strip query string from path before decode

Request header size limit
  └─> 400 Bad Request response (must be able to send 400 before limit is hit)

HEAD method support
  └─> Response struct with header/body separation (needed to send headers without body)

Date header
  └─> DateTime::now() with correct RFC 7231 format (DateTime module exists; needs HTTP format output)

Connection: close header
  └─> TCP connection close after write (already implied by current single-request design)
```

---

## MVP Feature Set

The minimum set that makes the server correct and safe:

**Must build (v1):**
1. Configurable static root directory (CLI arg) — fixes known crash bug
2. Binary-safe file reading (`fs::read`, byte buffers)
3. MIME type detection from extension (`.html`, `.css`, `.js`, `.wasm`, at minimum)
4. Path traversal prevention (percent-decode, canonicalize, prefix check)
5. Request header size limit (8KB max, return 400 if exceeded)
6. 400 Bad Request on malformed requests (fix crash bug)
7. 404 Not Found from generated body (not hardcoded file)
8. 500 Internal Server Error for I/O failures
9. `Content-Type`, `Content-Length`, `Date` headers on all responses
10. `Connection: close` header on all responses
11. HEAD method support (same headers, no body)
12. 405 Method Not Allowed for POST/PUT/DELETE to static paths
13. Query string stripping before file lookup
14. Remove the `/sleep` test endpoint

**Defer to v2:**
- Conditional GET (`If-Modified-Since`, `ETag`)
- `index.html` implicit serving for directory requests
- Range requests
- Configurable error pages

---

## HTTP/1.1 Compliance Requirements (Mandatory Headers per RFC)

For a server sending a `200 OK` response to a `GET` request, the following headers are mandatory
or strongly required by RFC 9110/9112:

| Header | Status | Rationale |
|--------|--------|-----------|
| `Content-Length` | REQUIRED (for non-chunked responses) | Message framing; RFC 9110 §8.6 |
| `Content-Type` | REQUIRED (for non-empty bodies) | MIME sniffing is a security risk; RFC 9110 §8.3 |
| `Date` | SHOULD (treated as MUST in practice) | Cache validation; RFC 9110 §6.6.1 |
| HTTP version `HTTP/1.1` in status line | REQUIRED | RFC 9112 §4 |

The following are NOT required by RFC but prevent common interoperability problems in practice:
- `Connection: close` — prevents clients from expecting keep-alive when the server does not support it
- `Server` — optional; add if desired but adds no correctness value

---

## Security Requirements Summary

| Threat | Mitigation | Priority |
|--------|------------|----------|
| Path traversal (`../../etc/passwd`) | Percent-decode path, then `fs::canonicalize`, then assert `starts_with(root)` | CRITICAL |
| Null byte injection in path | Reject any path containing `\0` before or after decoding | HIGH |
| Request header exhaustion | Hard limit on bytes read from request (8KB is conventional) | HIGH |
| DoS via blocking endpoint | Remove `/sleep` endpoint | HIGH |
| MIME sniffing attacks | Always send correct `Content-Type` with `charset=utf-8` for text types | HIGH |
| Binary file corruption | Use `fs::read` not `fs::read_to_string` | MEDIUM (correctness) |
| Thread panic on malformed input | Return 400 instead of unwrap/panic | MEDIUM |

---

## Sources

- RFC 9110: HTTP Semantics — https://www.rfc-editor.org/rfc/rfc9110 (mandatory headers, status codes, method semantics)
- RFC 9112: HTTP/1.1 — https://www.rfc-editor.org/rfc/rfc9112 (message framing, Content-Length, Host requirement)
- RFC 3986: URI Syntax — https://www.rfc-editor.org/rfc/rfc3986 (percent-encoding, path normalization)
- CONCERNS.md: Codebase audit identifying specific bugs and security gaps (HIGH confidence — direct code analysis)
- PROJECT.md: Scope and constraint definitions (HIGH confidence — authoritative project spec)
- Confidence level for RFC requirements: HIGH (stable specifications, well-understood)
- Confidence level for "conventional" recommendations (8KB header limit, `Connection: close`): MEDIUM (widely adopted practice, not normative)
