# Phase 5: Static File Serving - Research

**Researched:** 2026-03-02
**Domain:** Rust std — filesystem I/O, MIME type mapping, CLI argument parsing, path canonicalization
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

#### CLI argument design
- Root directory specified via `--root <path>` (long flag, no short form)
- Flag is **required** — server exits with a clear error message if omitted
- Validated at startup: path must exist and be a directory; exit with descriptive error if not
- No hardcoded default path in the binary

#### MIME type mapping
- Explicit mapping for: `.html` → `text/html`, `.css` → `text/css`, `.js` → `application/javascript`, `.wasm` → `application/wasm`, `.png` → `image/png`, `.jpg`/`.jpeg` → `image/jpeg`, `.gif` → `image/gif`, `.svg` → `image/svg+xml`, `.ico` → `image/x-icon`, `.txt` → `text/plain`
- Unknown/unmapped extensions fall back to `application/octet-stream` — serve the file rather than rejecting it

#### Missing file 404
- When a file does not exist under the root, return 404 with plain text "Not Found" body
- Consistent with the existing `NotFoundHandler` pattern (no custom 404.html served from disk)

#### Static file handler routing
- `StaticFileHandler` registered as a wildcard fallback in the router — invoked for any request path that has no exact route match
- Registered in `main.rs` after all exact routes (e.g., `GET /` continues to route to `RootHandler`)

#### HEAD request handling
- `StaticFileHandler` detects `HEAD` method: reads file metadata, sets `Content-Type`, `Content-Length`, and `Date` headers, but writes no body
- No separate handler needed; the same handler branches on method

#### PATH-03 (canonicalize check)
- `StaticFileHandler::handle()` calls `std::fs::canonicalize()` on the resolved path and verifies it starts with the canonicalized root
- Returns 404 if the canonical path escapes the root (covers symlinks and any OS-level traversal)
- This is the final layer of path safety after the router's `..`-component guard from Phase 4

#### Claude's Discretion
- Exact signature and field layout of `StaticFileHandler` struct (holds root `PathBuf`)
- Whether MIME detection lives as a free function or method on the handler
- Error mapping (file permission errors, read errors → 500 or 404)

### Deferred Ideas (OUT OF SCOPE)
- Directory listing — own phase
- Caching headers (ETag, Last-Modified, Cache-Control) — own phase
- Serving a custom 404.html from the root directory — future enhancement
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| FILE-01 | Server reads files with binary-safe `fs::read()` (not `read_to_string`) | `std::fs::read()` returns `Vec<u8>` — binary-safe by design; `Response::body(Vec<u8>)` already accepts binary bodies |
| FILE-02 | Server detects MIME type from file extension and sets `Content-Type` accordingly | `Path::extension()` returns the extension; a `match` on `extension.to_str()` covers all required MIME types; fallback to `application/octet-stream` |
| FILE-03 | Static file root directory is configurable via CLI argument at startup | `std::env::args()` parsed for `--root <path>` before `Router::new()`; validate with `Path::is_dir()` |
| FILE-04 | Server handles HEAD requests by returning headers only (no body) | `ctx.request.method == RequestMethod::Head` branch in `handle()`; set headers, skip `.body()` call |
| FILE-05 | Server returns 404 when a requested static file does not exist | `std::fs::read()` returns `Err(NotFound)` → write 404 using `NotFoundHandler` pattern and return `Ok(())` |
| PATH-03 | Server uses `canonicalize()` + `starts_with(root)` check to prevent path traversal, returning 404 | `std::fs::canonicalize()` on resolved path; `.starts_with(canonical_root)` check; failure → 404 via `Ok(())` |
</phase_requirements>

---

## Summary

Phase 5 adds `StaticFileHandler` — a fallback handler that maps request paths to files under a configurable root directory. The implementation is pure Rust std with zero new dependencies. The handler implements the existing `Handler` trait (`fn handle(&self, ctx: &mut Context) -> Result<()>`), holds a `PathBuf` for the root, and is registered in `main.rs` as the router's catch-all after exact routes.

The two main structural problems are: (1) routing fallback — the `Router` currently has no fallback slot; the CONTEXT.md defers this decision to the planner but the mechanism must be chosen, and (2) CLI argument parsing — `main()` currently takes no arguments, so `--root` parsing must be added before `Router::new()`. Both problems are solved entirely with Rust std: `std::env::args()` for CLI parsing and a fallback field on `Router` (or a sentinel path) for catch-all routing.

PATH-03 (`canonicalize` + `starts_with(root)`) is the third and final layer of path traversal defense. It completes the chain: Phase 4's router guard rejects `..` components structurally; Phase 5's handler applies `canonicalize()` to catch symlinks and any OS-level traversal that slipped past the structural check. Both layers must be present — they address different attack surfaces.

**Primary recommendation:** Add a `fallback: Option<Box<dyn Handler>>` field to `Router` with a `set_fallback()` builder method. `StaticFileHandler` holds a `PathBuf` root, implements `Handler`, is constructed with `StaticFileHandler::new(root)`, and is registered via `router.set_fallback(StaticFileHandler::new(root))` in `main.rs`.

---

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `std::fs::read()` | Rust std (edition 2021) | Binary-safe file reading → `Vec<u8>` | Returns `Vec<u8>` by design; cannot accidentally truncate at NUL bytes or invalid UTF-8 |
| `std::fs::canonicalize()` | Rust std (edition 2021) | Resolve symlinks + normalize path for traversal check | Only stdlib function that resolves the actual OS path; required for PATH-03 |
| `std::fs::metadata()` | Rust std (edition 2021) | Get file size for `Content-Length` on HEAD requests without reading file body | Avoids reading large files just to get the size for HEAD |
| `std::path::Path` / `PathBuf` | Rust std (edition 2021) | Path construction, extension detection, `starts_with` check | `Path::extension()` extracts the extension for MIME mapping; `PathBuf::join()` safely appends the decoded path |
| `std::env::args()` | Rust std (edition 2021) | CLI argument parsing for `--root <path>` | No external crate required; simple two-argument flag |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `std::io::ErrorKind::NotFound` | Rust std | Distinguish "file not found" (404) from "permission denied" or other I/O errors (500) | In the error mapping branch after `fs::read()` returns `Err` |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Manual `std::env::args()` parsing | `clap` or `argh` | External crate — project constraint is zero new deps; the flag is simple enough for manual parsing |
| `fallback` field on `Router` | Sentinel path (e.g., `"*"`) in `routes` vec | Sentinel requires the dispatch loop to treat one entry specially; a dedicated field is cleaner and more explicit |
| `fs::metadata()` for HEAD | `fs::read()` + empty body | Reading the full file just for HEAD is wasteful for large files; `metadata()` is O(1) at the OS level |

**Installation:** No changes to `Cargo.toml`. Zero new dependencies.

---

## Architecture Patterns

### Recommended Project Structure

```
src/
├── handlers/
│   └── mod.rs          # Add StaticFileHandler struct + impl Handler
├── server/
│   └── router.rs       # Add fallback field + set_fallback() builder method
└── main.rs             # Add --root CLI parsing + StaticFileHandler registration
```

### Pattern 1: StaticFileHandler Struct

**What:** A handler that holds the server root as a `PathBuf`. Construction validates the root at startup. The `handle()` method resolves the request path, performs the traversal check, reads the file, maps the MIME type, and writes the response.

**When to use:** Registered as the router fallback — invoked for any path that has no exact route match.

**Example:**
```rust
// Source: src/handlers/mod.rs (new addition)
use std::path::PathBuf;
use crate::server::{Context, Handler, RequestMethod, Response, Result};

pub struct StaticFileHandler {
    root: PathBuf,
}

impl StaticFileHandler {
    pub fn new(root: PathBuf) -> Self {
        StaticFileHandler { root }
    }
}

impl Handler for StaticFileHandler {
    fn handle(&self, ctx: &mut Context) -> Result<()> {
        // 1. Get the decoded path (already validated by Router guard — no '..' present)
        let decoded = ctx.request.target.decoded_path()?;

        // 2. Strip the leading '/' and join to root
        let rel = decoded.trim_start_matches('/');
        let candidate = self.root.join(rel);

        // 3. PATH-03: canonicalize + starts_with(root) check
        let canonical_root = std::fs::canonicalize(&self.root)?;
        let canonical = match std::fs::canonicalize(&candidate) {
            Ok(p) => p,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return not_found(ctx);
            }
            Err(e) => return Err(e.into()),
        };
        if !canonical.starts_with(&canonical_root) {
            return not_found(ctx);
        }

        // 4. Read file (binary-safe) — FILE-01
        let body = match std::fs::read(&canonical) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return not_found(ctx);
            }
            Err(e) => return Err(e.into()),
        };

        // 5. MIME type — FILE-02
        let content_type = mime_type(&canonical);
        let content_length = body.len().to_string();

        // 6. HEAD vs GET — FILE-04
        if ctx.request.method == RequestMethod::Head {
            ctx.response = Response::new(200, "OK")
                .header("Content-Type", content_type)
                .header("Content-Length", &content_length)
                .header("Connection", "close");
        } else {
            ctx.response = Response::new(200, "OK")
                .header("Content-Type", content_type)
                .header("Content-Length", &content_length)
                .header("Connection", "close")
                .body(body);
        }
        Ok(())
    }
}
```

### Pattern 2: MIME Type Detection

**What:** A free function (or private method) that takes a `&Path` and returns `&'static str`. Uses `Path::extension()` and matches on the lowercased extension string. Returns `"application/octet-stream"` for all unmapped extensions.

**When to use:** Called inside `StaticFileHandler::handle()` after the file path is validated.

**Example:**
```rust
// Source: std::path::Path::extension() — Rust std
fn mime_type(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html",
        Some("css")  => "text/css",
        Some("js")   => "application/javascript",
        Some("wasm") => "application/wasm",
        Some("png")  => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif")  => "image/gif",
        Some("svg")  => "image/svg+xml",
        Some("ico")  => "image/x-icon",
        Some("txt")  => "text/plain",
        _            => "application/octet-stream",
    }
}
```

Note: `Path::extension()` returns the part after the last `.`, already without the dot. Extension comparison is case-sensitive on this platform. The locked decisions list only lowercase extensions; no uppercase handling is required.

### Pattern 3: Router Fallback Slot

**What:** A `fallback: Option<Box<dyn Handler>>` field on `Router`. `dispatch()` checks this field after the route vec is exhausted. `set_fallback()` is a builder method that sets the field and returns `self`.

**When to use:** `StaticFileHandler` is registered here in `main.rs` so all unmatched paths reach it.

**Current Router state (from `src/server/router.rs`):**
```rust
pub struct Router {
    routes: Vec<(RequestMethod, String, Box<dyn Handler>)>,
    // Phase 5 adds: fallback: Option<Box<dyn Handler>>,
}
```

**Addition:**
```rust
pub struct Router {
    routes: Vec<(RequestMethod, String, Box<dyn Handler>)>,
    fallback: Option<Box<dyn Handler>>,
}

impl Router {
    pub fn new() -> Self {
        Router { routes: Vec::new(), fallback: None }
    }

    /// Set a fallback handler invoked when no route matches.
    /// Replaces the built-in NotFoundHandler for unmatched requests.
    pub fn set_fallback(mut self, handler: impl Handler + 'static) -> Self {
        self.fallback = Some(Box::new(handler));
        self
    }

    pub fn dispatch(&self, ctx: &mut Context) -> Result<()> {
        // ... existing safety guard (decoded_path, dotdot check) ...
        // ... existing route loop ...
        // After loop exhausted:
        match &self.fallback {
            Some(h) => h.handle(ctx),
            None => NotFoundHandler.handle(ctx),
        }
    }
}
```

**Alternative — method-chaining or mutable setter:**
`set_fallback(mut self, ...) -> Self` follows the builder pattern already used by `Server::with_router()`. A mutable `&mut self` version would also work; either is acceptable as Claude's Discretion.

### Pattern 4: CLI Argument Parsing in `main.rs`

**What:** Parse `std::env::args()` for `--root <path>` before any server construction. Exit with a descriptive error message if the flag is missing, if the value is absent, or if the path does not exist/is not a directory.

**When to use:** At the top of `main()` before `Router::new()`.

**Example:**
```rust
// Source: std::env::args() — Rust std
fn parse_root() -> crate::Result<std::path::PathBuf> {
    let args: Vec<String> = std::env::args().collect();
    let pos = args.iter().position(|a| a == "--root")
        .ok_or("--root <path> is required")?;
    let path_str = args.get(pos + 1)
        .ok_or("--root requires a path argument")?;
    let path = std::path::PathBuf::from(path_str);
    if !path.is_dir() {
        return Err(format!("--root '{}': not a directory or does not exist", path_str).into());
    }
    Ok(path)
}
```

This returns a `crate::Result<PathBuf>` — the top-level `main()` already returns `Result<()>`, so `?` propagates cleanly. The error message is printed to stderr via the `Result<()>` main pattern and the process exits non-zero.

### Pattern 5: 404 Helper for StaticFileHandler

**What:** A private free function `fn not_found(ctx: &mut Context) -> Result<()>` that mirrors `NotFoundHandler` in `router.rs`. Avoids importing the private `NotFoundHandler` across modules.

**When to use:** Called from `StaticFileHandler::handle()` when the file is not found or the traversal check fails.

**Example:**
```rust
// Pattern matches existing NotFoundHandler in src/server/router.rs
fn not_found(ctx: &mut Context) -> crate::server::Result<()> {
    let body = b"Not Found".to_vec();
    let content_length = body.len().to_string();
    ctx.response = crate::server::Response::new(404, "Not Found")
        .header("Content-Type", "text/plain")
        .header("Content-Length", &content_length)
        .header("Connection", "close")
        .body(body);
    Ok(())
}
```

Alternatively, `NotFoundHandler` in `router.rs` could be made `pub(crate)` and imported. Either approach is Claude's Discretion — the inline function keeps `handlers/mod.rs` self-contained without touching `router.rs`.

### Anti-Patterns to Avoid

- **Using `fs::read_to_string()` for file body:** Fails on binary files (images, WASM). `fs::read()` is required (FILE-01). Never use `read_to_string` for file serving.
- **Returning `Err` from `handle()` for missing files (404 case):** `Err` from `handle()` causes `handle_connection` to send 500. A missing file is a 404, not a server error — write the 404 and return `Ok(())`.
- **Returning `Err` from `handle()` for traversal rejection (PATH-03):** Same as above — write 404, return `Ok(())`.
- **Skipping `canonicalize()` and relying only on the router's `..` guard:** The router guard catches structural `..` components but cannot catch symlinks that escape the root. Both defenses are required.
- **Calling `canonicalize()` before checking file existence:** `canonicalize()` returns `Err(NotFound)` if the file does not exist — which is the correct 404 signal. Handle `NotFound` as a 404, all other `canonicalize()` errors as 500.
- **Joining the raw (encoded) path to root:** The path used for `join()` must be the decoded path (already available from `ctx.request.target.decoded_path()`). The router's guard has already validated there are no `..` components in the decoded form.
- **Leading slash in `join()`:** `PathBuf::join()` with an absolute path (starts with `/`) replaces the base — `"/".join("/etc/passwd")` is `/etc/passwd`, not `root/etc/passwd`. Always `trim_start_matches('/')` before joining.
- **Case-sensitive extension matching missing `jpeg`:** The MIME map must match both `"jpg"` and `"jpeg"` as separate arms. `Path::extension()` returns the full extension after the last dot — `file.jpeg` gives `"jpeg"`, not `"jpg"`.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Binary-safe file reading | Custom buffered reader | `std::fs::read()` | Returns `Vec<u8>` — already binary-safe, already in std |
| Path traversal prevention | String scanning for ".." sequences | `std::fs::canonicalize()` + `starts_with()` | Handles symlinks, OS-level traversal, and all path normalization edge cases that string scanning misses |
| File size for HEAD | Read file, count bytes | `std::fs::metadata().len()` | O(1) OS syscall; does not load the file into memory |
| CLI flag parsing | Custom tokenizer | Simple `args().position()` loop | One required flag — the added complexity of a library is not justified; no new crates allowed |
| MIME type library | External crate | Static `match` on `Path::extension()` | Only 10 types required by the locked decisions; a static match is 15 lines and has zero edge cases |

**Key insight:** Every "don't hand-roll" item in this domain is already in Rust std. External crates would add complexity and dependency surface area without benefit for this scope.

---

## Common Pitfalls

### Pitfall 1: Leading Slash in PathBuf::join()

**What goes wrong:** `root.join("/etc/passwd")` returns `PathBuf::from("/etc/passwd")` — the root is silently discarded because the joined component is absolute.
**Why it happens:** `PathBuf::join()` matches POSIX path behavior: joining an absolute path replaces the base.
**How to avoid:** Strip the leading `/` from the decoded path before joining: `let rel = decoded.trim_start_matches('/'); root.join(rel)`.
**Warning signs:** A test for `GET /etc/passwd` reaches a file outside the root; `canonicalize()` returns a path that doesn't start with the canonical root.

### Pitfall 2: canonicalize() Returns Err(NotFound) for Non-Existent Files

**What goes wrong:** `canonicalize()` is called before checking file existence. If the file doesn't exist, `canonicalize()` returns `Err(NotFound)`. Propagating this with `?` converts it to a 500 instead of a 404.
**Why it happens:** `canonicalize()` must follow the full path to resolve symlinks — it requires the path to exist.
**How to avoid:** Match on the error kind: `Err(e) if e.kind() == ErrorKind::NotFound => return not_found(ctx)`. All other errors from `canonicalize()` (e.g., permission denied) propagate as 500 via `?`.
**Warning signs:** A request for a non-existent file returns 500 instead of 404.

### Pitfall 3: Returning Err for 404 Conditions

**What goes wrong:** The handler returns `Err(...)` when a file is not found or the traversal check fails. The `handle_connection` caller catches all `Err` from `dispatch()` and sends a 500. The correct 404 is overwritten with 500.
**Why it happens:** Treating "file not found" and "traversal rejected" as handler errors rather than normal HTTP conditions.
**How to avoid:** Follow the `NotFoundHandler` pattern — write the 404 response into `ctx.response` and return `Ok(())` for all 404 conditions. Return `Err` only for unexpected I/O errors (which correctly become 500).
**Warning signs:** Any test for a non-existent or traversal path returns 500.

### Pitfall 4: HEAD Response Includes a Body

**What goes wrong:** HEAD requests are handled identically to GET — the body is included in the response. Clients that send HEAD expect a headers-only response.
**Why it happens:** Forgetting to branch on `ctx.request.method` before calling `.body()`.
**How to avoid:** Check `ctx.request.method == RequestMethod::Head` before setting the body. On HEAD: set `Content-Type`, `Content-Length`, `Connection` headers, do NOT call `.body()`. On GET: set headers and call `.body(body)`.
**Warning signs:** A HEAD request response contains bytes after the `\r\n\r\n` separator.

### Pitfall 5: canonicalize() on Root Fails at Startup

**What goes wrong:** `canonicalize()` is called per-request on `self.root`. If the root directory is deleted after startup, `canonicalize()` fails on every request. More critically, if the root path has symlinks or is specified as a relative path, `canonicalize()` at request time may return different results than expected.
**Why it happens:** Not canonicalizing the root at construction time.
**How to avoid:** Canonicalize the root once in `StaticFileHandler::new()` and store `canonical_root: PathBuf` as a field. Per-request, only `canonicalize()` the candidate file path and compare against the pre-computed `self.canonical_root`.
**Warning signs:** Path traversal test with symlink targets passes when it should fail; or performance issue from repeated root canonicalization.

### Pitfall 6: fs::read() Failure Not Differentiated

**What goes wrong:** `fs::read()` returns `Err` for multiple reasons: `NotFound` (404), `PermissionDenied` (could be 403/500), other I/O errors (500). Treating all errors as 500 gives wrong status for missing files.
**Why it happens:** Using `?` on `fs::read()` without matching the error kind first.
**How to avoid:** Match `Err(e) if e.kind() == ErrorKind::NotFound => return not_found(ctx)` before `?`. This is the same pattern as for `canonicalize()` errors.
**Warning signs:** A request for `/nonexistent.txt` returns 500 instead of 404.

### Pitfall 7: Router's Existing NotFoundHandler Intercepts Static Files

**What goes wrong:** After registering `StaticFileHandler` as a fallback, the `NotFoundHandler` built into `dispatch()` still fires for any path not in the routes vec, before the fallback is reached.
**Why it happens:** The current `dispatch()` always calls `NotFoundHandler.handle(ctx)` at the end with no fallback check.
**How to avoid:** Add a `fallback: Option<Box<dyn Handler>>` field to `Router`. At the end of `dispatch()`, call `fallback.handle(ctx)` if set, otherwise call `NotFoundHandler`. This preserves the existing behavior when no fallback is registered.
**Warning signs:** A static file request returns 404 even though the file exists under the root and `StaticFileHandler` is registered.

---

## Code Examples

Verified patterns from official sources and existing codebase:

### Binary-Safe File Read (FILE-01)

```rust
// Source: std::fs::read() — returns Vec<u8>, binary-safe
use std::fs;
use std::io::ErrorKind;

let body: Vec<u8> = match fs::read(&file_path) {
    Ok(bytes) => bytes,
    Err(e) if e.kind() == ErrorKind::NotFound => {
        return not_found(ctx);
    }
    Err(e) => return Err(e.into()),
};
```

### MIME Type from Path Extension (FILE-02)

```rust
// Source: std::path::Path::extension()
fn mime_type(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html",
        Some("css")  => "text/css",
        Some("js")   => "application/javascript",
        Some("wasm") => "application/wasm",
        Some("png")  => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif")  => "image/gif",
        Some("svg")  => "image/svg+xml",
        Some("ico")  => "image/x-icon",
        Some("txt")  => "text/plain",
        _            => "application/octet-stream",
    }
}
```

### PATH-03: canonicalize() + starts_with() Check

```rust
// Source: std::fs::canonicalize() — Rust std
// canonical_root is pre-computed in StaticFileHandler::new()
let canonical = match std::fs::canonicalize(&candidate) {
    Ok(p) => p,
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
        return not_found(ctx);
    }
    Err(e) => return Err(e.into()),
};
if !canonical.starts_with(&self.canonical_root) {
    return not_found(ctx); // traversal attempt — return 404, never 403
}
```

### HEAD vs GET Body Branch (FILE-04)

```rust
// Source: existing RequestMethod::Head variant in src/server/request.rs
use crate::server::RequestMethod;

if ctx.request.method == RequestMethod::Head {
    ctx.response = Response::new(200, "OK")
        .header("Content-Type", content_type)
        .header("Content-Length", &content_length)
        .header("Connection", "close");
    // No .body() call — HEAD must not include a response body
} else {
    ctx.response = Response::new(200, "OK")
        .header("Content-Type", content_type)
        .header("Content-Length", &content_length)
        .header("Connection", "close")
        .body(body);
}
```

### Leading-Slash Strip Before join()

```rust
// Source: std::path::PathBuf::join() behavior — absolute paths replace base
// MUST strip leading '/' before joining to prevent PathBuf::join from discarding root
let decoded = ctx.request.target.decoded_path()?;
let rel = decoded.trim_start_matches('/');
let candidate = self.root.join(rel); // root: /srv/files, rel: index.html -> /srv/files/index.html
```

### Router Fallback Slot Addition

```rust
// Addition to src/server/router.rs
pub struct Router {
    routes: Vec<(RequestMethod, String, Box<dyn Handler>)>,
    fallback: Option<Box<dyn Handler>>,  // NEW: catch-all for unmatched routes
}

impl Router {
    pub fn new() -> Self {
        Router { routes: Vec::new(), fallback: None }
    }

    /// Register a fallback handler (value-chaining builder, matches Server::with_router pattern).
    pub fn set_fallback(mut self, handler: impl Handler + 'static) -> Self {
        self.fallback = Some(Box::new(handler));
        self
    }

    pub fn dispatch(&self, ctx: &mut Context) -> Result<()> {
        let decoded = match ctx.request.target.decoded_path() {
            Ok(d) => d,
            Err(_) => return NotFoundHandler.handle(ctx),
        };
        if decoded.split('/').any(|c| c == "..") {
            return NotFoundHandler.handle(ctx);
        }
        let method = &ctx.request.method;
        for (route_method, route_path, handler) in &self.routes {
            if route_method == method && route_path.as_str() == decoded.as_str() {
                return handler.handle(ctx);
            }
        }
        // Fallback: StaticFileHandler (if registered) or built-in 404
        match &self.fallback {
            Some(h) => h.handle(ctx),
            None => NotFoundHandler.handle(ctx),
        }
    }
}
```

### main.rs Integration

```rust
// Addition to src/main.rs
fn main() -> Result<()> {
    SimpleLogger::init()?;
    let root = parse_root()?;  // exits with error if --root missing or invalid
    let handler = handlers::StaticFileHandler::new(root)?;  // canonicalizes root at construction
    let mut router = Router::new();
    router.add("GET", "/", RootHandler)?;
    let router = router.set_fallback(handler);
    Server::new(DEFAULT_ADDR)?.with_router(router).run()?;
    Ok(())
}
```

---

## State of the Art

| Old Approach | Current Approach | Impact |
|--------------|-----------------|--------|
| No static file serving — only `RootHandler` on `GET /` | `StaticFileHandler` as router fallback | Any file under root is served by default |
| Router always falls through to built-in `NotFoundHandler` | Router has optional `fallback: Option<Box<dyn Handler>>` | Catch-all handler pattern without special-casing in dispatch loop |
| `main.rs` takes no arguments | `--root <path>` CLI flag validated at startup | Root is configurable; server refuses to start without a valid directory |
| PATH-03 deferred from Phase 4 (no root available) | `canonicalize()` + `starts_with(canonical_root)` in `StaticFileHandler::handle()` | Third traversal defense layer completes the defense-in-depth chain |

**Not deprecated:**
- Phase 4's router `..`-component guard: still required as the first structural defense. Both Phase 4 (structural) and Phase 5 (filesystem) guards must coexist.

---

## Open Questions

1. **Should `StaticFileHandler::new()` return `Result<Self>` (for root canonicalization at construction) or `Self` (defer canonicalization to first request)?**
   - What we know: Canonicalizing the root once at construction is correct (avoids per-request overhead, fails fast at startup). `std::fs::canonicalize()` returns `Result` and can fail.
   - What's unclear: Whether `StaticFileHandler::new()` should return `Result<Self>` (requires `?` at the call site in `main.rs`) or store the raw `PathBuf` and canonicalize per-request.
   - Recommendation: Return `Result<Self>` from `new()`. Early failure at startup is the correct behavior — if the root can't be canonicalized, the server should not start. The call site in `main.rs` already handles `Result` via `?`.

2. **Router `set_fallback()` — value-chaining builder or mutable setter?**
   - What we know: `Server::with_router()` uses value-chaining (`self` → `Self`). `Router::add()` uses `&mut self`. The locked decision says `StaticFileHandler` is registered as a fallback after exact routes.
   - What's unclear: Which mutation style best fits `main.rs` ergonomics.
   - Recommendation: Value-chaining (`set_fallback(mut self, ...) -> Self`) to match `Server::with_router()`. Alternatively, `add_fallback(&mut self, ...)` to match `Router::add()`. Either is correct — planner chooses.

3. **What happens for GET on a directory path (e.g., `GET /images/`)? No `is_dir()` check is specified.**
   - What we know: Directory listing is deferred. The `fs::read()` call on a directory path returns `Err(IsADirectory)` on Linux or `Err(PermissionDenied)` on some systems.
   - What's unclear: Whether the error kind is platform-consistent and whether it should map to 404 or 500.
   - Recommendation: Map non-NotFound read errors (including directory read errors) to 500 via `?`. This is acceptable — directory listing is out of scope and a 500 for a directory path is better than a wrong MIME type or panic. Alternatively, add an explicit `if candidate.is_dir() { return not_found(ctx); }` check before `fs::read()` for cleaner 404 behavior on directory paths.

---

## Sources

### Primary (HIGH confidence)

- Source code: `src/server/router.rs` — current `Router` struct, `dispatch()`, `NotFoundHandler` pattern; read directly
- Source code: `src/handlers/mod.rs` — `RootHandler`, `Handler` trait usage, response builder pattern; read directly
- Source code: `src/server/request.rs` — `RequestMethod::Head` variant confirmed present; read directly
- Source code: `src/server/response.rs` — `Response::body(Vec<u8>)` accepts binary; `write_to` does not add body for empty `Vec`; read directly
- Source code: `src/url/mod.rs` — `Url::decoded_path()` signature and return type confirmed; read directly
- Source code: `src/main.rs` — current structure, `Result<()>` return from `main()`, `Router::new()` call site; read directly
- Rust std: `std::fs::read()` — returns `Vec<u8>`, binary-safe; `std::fs::canonicalize()` — resolves symlinks; `std::path::Path::extension()` — returns extension OsStr; `PathBuf::join()` replaces base on absolute input; all confirmed from direct code reading and Rust std documentation

### Secondary (MEDIUM confidence)

- [StackHawk Rust Path Traversal Guide](https://www.stackhawk.com/blog/rust-path-traversal-guide-example-and-prevention/) — confirms `canonicalize()` + `starts_with()` as the canonical Rust traversal check; consistent with Phase 4 research findings
- [OWASP Path Traversal](https://owasp.org/www-community/attacks/Path_Traversal) — confirms 404 (not 403) for traversal rejection avoids leaking detection; consistent with REQUIREMENTS.md "Out of Scope" table entry

### Tertiary (LOW confidence)

- None — all critical patterns verified from source code directly.

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new dependencies; all std primitives confirmed by reading the existing source code and prior phase research
- Architecture: HIGH — patterns derived directly from the existing codebase (Handler trait, Response builder, NotFoundHandler, RequestMethod::Head, Router dispatch structure)
- Pitfalls: HIGH — PathBuf::join absolute-path behavior and canonicalize() NotFound handling verified from Rust std semantics; traversal pattern confirmed from Phase 4 research

**Research date:** 2026-03-02
**Valid until:** 2026-04-02 (stable domain — Rust std does not change; patterns are structural)
