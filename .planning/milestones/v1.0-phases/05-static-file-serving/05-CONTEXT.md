# Phase 5: Static File Serving - Context

**Gathered:** 2026-03-02
**Status:** Ready for planning

<domain>
## Phase Boundary

Serve any file found under a configurable root directory: binary-safe reads, correct Content-Type headers, configurable root via CLI argument, correct HEAD handling, and canonicalize-based path traversal guard (PATH-03). Creating files, directory listings, and caching are separate phases.

</domain>

<decisions>
## Implementation Decisions

### CLI argument design
- Root directory specified via `--root <path>` (long flag, no short form)
- Flag is **required** — server exits with a clear error message if omitted
- Validated at startup: path must exist and be a directory; exit with descriptive error if not
- No hardcoded default path in the binary

### MIME type mapping
- Explicit mapping for: `.html` → `text/html`, `.css` → `text/css`, `.js` → `application/javascript`, `.wasm` → `application/wasm`, `.png` → `image/png`, `.jpg`/`.jpeg` → `image/jpeg`, `.gif` → `image/gif`, `.svg` → `image/svg+xml`, `.ico` → `image/x-icon`, `.txt` → `text/plain`
- Unknown/unmapped extensions fall back to `application/octet-stream` — serve the file rather than rejecting it

### Missing file 404
- When a file does not exist under the root, return 404 with plain text "Not Found" body
- Consistent with the existing `NotFoundHandler` pattern (no custom 404.html served from disk)

### Static file handler routing
- `StaticFileHandler` registered as a wildcard fallback in the router — invoked for any request path that has no exact route match
- Registered in `main.rs` after all exact routes (e.g., `GET /` continues to route to `RootHandler`)

### HEAD request handling
- `StaticFileHandler` detects `HEAD` method: reads file metadata, sets `Content-Type`, `Content-Length`, and `Date` headers, but writes no body
- No separate handler needed; the same handler branches on method

### PATH-03 (canonicalize check)
- `StaticFileHandler::handle()` calls `std::fs::canonicalize()` on the resolved path and verifies it starts with the canonicalized root
- Returns 404 if the canonical path escapes the root (covers symlinks and any OS-level traversal)
- This is the final layer of path safety after the router's `..`-component guard from Phase 4

### Claude's Discretion
- Exact signature and field layout of `StaticFileHandler` struct (holds root `PathBuf`)
- Whether MIME detection lives as a free function or method on the handler
- Error mapping (file permission errors, read errors → 500 or 404)

</decisions>

<specifics>
## Specific Ideas

- The existing `hello.html` and `404.html` at the project root are test/template files, not server-generated responses — the static file handler should serve them only if they fall within the configured root
- `fs::read()` (binary-safe) is explicitly required — `read_to_string` must not be used for file content

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `Handler` trait (`src/server/handler.rs`): `StaticFileHandler` implements this — `fn handle(&self, ctx: &mut Context) -> Result<()>`
- `Context` struct: `ctx.request.method` (for HEAD detection), `ctx.request.target.decoded_path()` (already percent-decoded by router guard), `ctx.response` (written in place)
- `Response` builder (`src/server/response.rs`): `.header()` chain + `.body(Vec<u8>)` — binary body already supported
- `NotFoundHandler` pattern: return 404 via `Response::new(404, "Not Found")` — follow same structure

### Established Patterns
- Handlers live in `src/handlers/mod.rs`; registered in `main.rs` via `router.add()`
- Binary-safe I/O with `Vec<u8>` is the established body type in `Response`
- Error propagation: `Err` from `handle()` causes 500; use `Ok(())` with a 404 response for expected missing-file cases

### Integration Points
- `main.rs`: Add `--root` CLI arg parsing before `Router::new()`; pass root `PathBuf` to `StaticFileHandler::new(root)`
- `router.rs`: Wildcard/fallback registration for `StaticFileHandler` — needs a mechanism to register a catch-all (currently exact-match only; planner should decide whether to add a fallback slot to `Router` or register with a sentinel path)
- `src/handlers/mod.rs`: Add `StaticFileHandler` struct alongside `RootHandler`

</code_context>

<deferred>
## Deferred Ideas

- Directory listing — own phase
- Caching headers (ETag, Last-Modified, Cache-Control) — own phase
- Serving a custom 404.html from the root directory — future enhancement

</deferred>

---

*Phase: 05-static-file-serving*
*Context gathered: 2026-03-02*
