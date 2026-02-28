# Architecture Research: Router + Static File Handler

**Research Date:** 2026-02-28
**Scope:** Extending the existing pure-Rust HTTP/1.1 server with a Router, Handler trait, and static file serving — no external dependencies beyond stdlib + `log`.

---

## Question

How should a Router and static file handler be structured in a pure-Rust HTTP server? How do Request → Router → Handler → Response integrate? What is the right abstraction for handlers that works for both static files and future REST endpoints?

---

## Summary

The existing server hardcodes routing inside `handle_connection()` with a `match` on the raw request line string and constructs HTTP responses inline as format strings. Both behaviors must be replaced by proper abstractions before adding any real features.

The path forward is:

1. Build a `Response` struct first — everything else produces one.
2. Define a `Handler` trait that takes a `&Request` and returns a `Result<Response>`.
3. Build a `Router` that maps `(method, path pattern)` to a boxed `Handler`.
4. Implement `StaticFileHandler` as the first concrete handler — it satisfies the same `Handler` trait that future REST handlers will use.
5. Wire `handle_connection()` to call `router.dispatch(&request)` and write the resulting `Response` to the stream.

This order ensures every new piece has something concrete to build on, and the `Handler` trait is the single extension point for both static files and REST.

---

## Component Boundaries

### Response (`src/server/response.rs`)

The `Response` struct replaces the inline format string in `handle_connection()`. It must own:

- A status code (`u16`) and reason phrase (`&'static str` or `String`)
- A header map (ordered `Vec<(String, String)>` is fine — no `HashMap` needed for HTTP/1.1 where header order can matter for debugging)
- A body (`Vec<u8>` — not `String`, so binary files work correctly)

It needs a single method that serializes the full response to bytes for writing to the stream. It also needs convenience constructors for common cases: `Response::ok()`, `Response::not_found()`, `Response::internal_error()`.

Example shape:

```rust
// src/server/response.rs
pub struct Response {
    pub status: u16,
    pub reason: &'static str,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Response {
    pub fn new(status: u16, reason: &'static str) -> Self { ... }
    pub fn set_header(&mut self, name: impl Into<String>, value: impl Into<String>) { ... }
    pub fn set_body(&mut self, body: Vec<u8>) { ... }
    pub fn to_bytes(&self) -> Vec<u8> { ... }
}
```

The `to_bytes()` method writes the status line, then each header as `Name: Value\r\n`, then `\r\n`, then the body bytes. `Content-Length` must always be set before calling `to_bytes()` — the handler is responsible for this, not `to_bytes()` itself (keep it a dumb serializer).

### Handler Trait (`src/server/handler.rs`)

The `Handler` trait is the central extension point. Its contract is: given a parsed `Request`, produce a `Response` or a `server::Error`.

```rust
// src/server/handler.rs
pub trait Handler: Send + Sync {
    fn handle(&self, request: &Request) -> Result<Response>;
}
```

`Send + Sync` bounds are required because handlers are stored in the `Router` which is shared across threads (via `Arc`). Handlers must be stateless or use interior mutability; the static file handler is naturally stateless (it reads from disk on every request).

Using `&self` (shared reference) rather than `&mut self` enforces this: the router hands the same handler to multiple threads concurrently. Rust's type system enforces thread safety here with no runtime cost.

`Box<dyn Handler>` is the storage type — no generics needed in the router, and this avoids monomorphization complexity for a project of this scale.

### Router (`src/server/router.rs`)

The `Router` holds a list of route entries, each pairing a `(RequestMethod, path pattern)` with a `Box<dyn Handler>`. Dispatch walks the list in registration order and calls the first matching handler.

For this project, exact string matching on the path is sufficient for now. A pattern type can be added later (regex, prefix, wildcard) without changing the `Handler` trait.

```rust
// src/server/router.rs
pub struct Router {
    routes: Vec<Route>,
    fallback: Box<dyn Handler>,
}

struct Route {
    method: RequestMethod,
    pattern: String,      // exact match for now
    handler: Box<dyn Handler>,
}

impl Router {
    pub fn new(fallback: Box<dyn Handler>) -> Self { ... }
    pub fn add(&mut self, method: RequestMethod, pattern: impl Into<String>, handler: Box<dyn Handler>) { ... }
    pub fn dispatch(&self, request: &Request) -> Result<Response> { ... }
}
```

The fallback handler is what gets called when no route matches — this is where `NotFoundHandler` lives, which always returns a 404. Having fallback as a `Box<dyn Handler>` (rather than a special case) means it participates in the same interface.

The `dispatch()` method:

1. Iterates `routes` in order.
2. Compares `route.method` with `request.method` and `route.pattern` with `request.target.path()`.
3. On first match, calls `route.handler.handle(request)` and returns.
4. If no route matches, calls `self.fallback.handle(request)`.

The `Router` should be wrapped in `Arc<Router>` at the `Server` level so it can be cloned into each thread's closure cheaply.

### Static File Handler (`src/server/static_files.rs`)

`StaticFileHandler` is a concrete `Handler` implementation. It holds the server root directory as a `PathBuf` and, on each request, resolves the requested path against the root, validates it, reads the file, and returns a `Response` with the appropriate `Content-Type` and `Content-Length`.

```rust
// src/server/static_files.rs
pub struct StaticFileHandler {
    root: PathBuf,
}

impl StaticFileHandler {
    pub fn new(root: impl Into<PathBuf>) -> Self { ... }
}

impl Handler for StaticFileHandler {
    fn handle(&self, request: &Request) -> Result<Response> { ... }
}
```

Key responsibilities of `handle()`:

1. Extract the decoded path from `request.target` (URL decode percent-encoded segments).
2. Resolve the path against `self.root` using `root.join(decoded_path)`.
3. Call `canonicalize()` on the resolved path and verify the result starts with `self.root.canonicalize()` — this is the path traversal prevention check.
4. Read the file with `fs::read()` (returns `Vec<u8>`, correct for binary).
5. Detect MIME type from the file extension.
6. Build and return a `Response` with status 200, `Content-Type`, `Content-Length`, and the body.

If the file does not exist, return a 404 response (not an `Err` — file-not-found is a handled case, not a server error). If the path resolves outside the root, return 403. If reading fails for other reasons, propagate the `Err`.

### MIME Type Detection (`src/server/mime.rs` or inline in `static_files.rs`)

A pure function mapping file extension to MIME type string. No external crates needed — a `match` on `Path::extension()` covers all required types:

```rust
fn mime_for_extension(ext: &str) -> &'static str {
    match ext {
        "html" | "htm" => "text/html; charset=utf-8",
        "css"          => "text/css; charset=utf-8",
        "js"           => "application/javascript",
        "json"         => "application/json",
        "wasm"         => "application/wasm",
        "png"          => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif"          => "image/gif",
        "svg"          => "image/svg+xml",
        "ico"          => "image/x-icon",
        "txt"          => "text/plain; charset=utf-8",
        _              => "application/octet-stream",
    }
}
```

This can live in `static_files.rs` as a private function or in a small `src/server/mime.rs` submodule if it grows.

### Not Found Handler (inline or `src/server/handler.rs`)

A trivial struct that always returns 404. Used as the router's fallback.

```rust
pub struct NotFoundHandler;

impl Handler for NotFoundHandler {
    fn handle(&self, _request: &Request) -> Result<Response> {
        let mut r = Response::new(404, "Not Found");
        let body = b"404 Not Found".to_vec();
        r.set_header("Content-Length", body.len().to_string());
        r.set_header("Content-Type", "text/plain; charset=utf-8");
        r.set_body(body);
        Ok(r)
    }
}
```

This replaces the hardcoded `404.html` file path in the current `handle_connection()`.

---

## Data Flow: Request to Response

```
TcpStream
    |
    v
handle_connection()
    |
    +-- BufReader reads HTTP headers (lines until blank line)
    |
    v
Request::parse(&raw_lines)
    |   - validates HTTP/1.1
    |   - parses method via RequestMethod::try_from()
    |   - parses target via Url::from()
    |   -> Result<Request>
    |
    v
router.dispatch(&request)           // Arc<Router> shared across threads
    |
    +-- iterates routes in order
    |   - matches method + path pattern
    |   - calls matched handler.handle(&request)
    |
    +-- if no route matches: fallback.handle(&request)
    |
    v
Result<Response>
    |
    +-- Err: construct a 500 Internal Server Error Response
    |
    v
response.to_bytes()
    |
    v
stream.write_all(&bytes)
    |
    v
Connection closed (HTTP/1.1 without Keep-Alive)
```

All error handling uses `Result<Response>` — the `?` operator propagates `server::Error` up from `dispatch()`. The caller (`handle_connection()`) converts any `Err` into a 500 response before writing, so the stream always gets a valid HTTP response.

---

## Handler Trait Design for Static Files and REST

The `Handler` trait is intentionally minimal:

```rust
pub trait Handler: Send + Sync {
    fn handle(&self, request: &Request) -> Result<Response>;
}
```

This works for both use cases:

**Static files:** `StaticFileHandler::handle()` reads the filesystem. Its state (the root `PathBuf`) is set at startup and never changes. `Send + Sync` is satisfied trivially.

**Future REST endpoints:** A REST handler might hold a `Arc<Mutex<State>>` for shared mutable data, or be a pure function on the request. Both fit `Handler`. Examples:

```rust
// Pure function handler
pub struct EchoHandler;
impl Handler for EchoHandler {
    fn handle(&self, request: &Request) -> Result<Response> {
        // echo request info back as JSON
    }
}

// Handler with shared state
pub struct CounterHandler {
    count: Arc<Mutex<u64>>,
}
impl Handler for CounterHandler {
    fn handle(&self, _: &Request) -> Result<Response> {
        let mut n = self.count.lock()?;
        *n += 1;
        // return n as response
    }
}
```

The router does not know or care what the handler does — it only calls `handle()`. This is the right level of abstraction: handlers are self-contained units that map a request to a response.

**Why not closures?** Closures (`Box<dyn Fn(&Request) -> Result<Response> + Send + Sync>`) would also work and are more concise. The `Handler` trait is preferable here because:

- It gives the type a name, which aids error messages and documentation.
- Struct-based handlers can hold configuration (like `StaticFileHandler`'s root).
- A trait can have additional methods in the future (e.g., `fn can_handle(&self, request: &Request) -> bool`) without changing call sites.
- It matches the existing codebase's pattern of named structs implementing standard traits (`Request`, `ThreadPool`, `SimpleLogger`).

---

## URL Integration

The existing `Url` struct in `src/url/mod.rs` currently stores only a raw path string. Before the `Router` and `StaticFileHandler` can work correctly, `Url` needs two capabilities:

1. **Path accessor:** A method `fn path(&self) -> &str` that returns the path component, stripped of any query string or fragment.
2. **Percent-decode:** A method `fn decoded_path(&self) -> Result<String>` that applies the existing `pct_decode()` logic to the path segments.

The router uses `path()` for pattern matching (compare raw path strings). The static file handler uses `decoded_path()` when resolving to a filesystem path. This keeps routing fast (no allocation) and file serving correct (percent-encoded characters in filenames are handled).

The `Url` module already has all the encoding/decoding logic — it just needs to be wired into the struct's public API, and the `#![allow(unused)]` override can be removed.

---

## Integration Points with Existing Code

| Existing item | How the new code connects |
|---|---|
| `Request` in `src/server/request.rs` | Passed as `&Request` into `Handler::handle()` and `Router::dispatch()`. No changes needed to `Request` itself. |
| `RequestMethod` in `src/server/request.rs` | Used in `Route` struct for method matching. `Router::add()` takes a `RequestMethod`. |
| `Url` in `src/url/mod.rs` | Needs `path()` and `decoded_path()` methods added. Router uses `path()`. Static file handler uses `decoded_path()`. |
| `handle_connection()` in `src/server/mod.rs` | Replaces the match+format block with: `let response = router.dispatch(&request).unwrap_or_else(|_| internal_error_response()); stream.write_all(&response.to_bytes())?;` |
| `Server` in `src/server/mod.rs` | Gains an `Arc<Router>` field. `Server::new()` (or a new `Server::with_router()`) accepts the router. The router is cloned into each thread's closure. |
| `server::error::Error` in `src/server/error.rs` | May need a new variant (e.g., `PathTraversal`, `NotFound`) or these can remain as `Io` variants with descriptive messages. A `NotFound` variant lets the router return 404 by convention without a special type. |
| `log` crate macros | Used throughout handler implementations for request/response logging, consistent with existing patterns. |

---

## Build Order (Dependencies)

The components have a strict dependency order. Each phase must be fully working before the next begins.

### Phase 1: Response struct

**Build:** `src/server/response.rs` with `Response` struct, `set_header()`, `set_body()`, `to_bytes()`, and convenience constructors.

**Why first:** Every other component produces a `Response`. Nothing else can be tested without it.

**Integration:** Replace the inline format string in `handle_connection()` with a `Response` constructed and serialized. No router yet — the existing match logic still runs, but now builds a `Response` instead of strings.

### Phase 2: Handler trait + NotFoundHandler

**Build:** `src/server/handler.rs` with the `Handler` trait and a `NotFoundHandler` struct.

**Depends on:** Phase 1 (`Response`).

**Why second:** The trait establishes the interface. Everything else implements it.

### Phase 3: Router

**Build:** `src/server/router.rs` with `Router` struct, `Route`, `add()`, `dispatch()`.

**Depends on:** Phase 2 (`Handler`), existing `Request` and `RequestMethod`.

**Integration:** Wire `Arc<Router>` into `Server`. Replace the match block in `handle_connection()` with `router.dispatch(&request)`. At this point the server should behave identically to before (same hardcoded routes, now via router entries) but with the new architecture in place.

### Phase 4: URL path and decode methods

**Build:** Add `path()` and `decoded_path()` to `Url` in `src/url/mod.rs`. Remove `#![allow(unused)]`.

**Depends on:** Existing percent-encoding helpers in `url/mod.rs`.

**Why here:** Needed by the static file handler but not by the router for exact matching. Can be done before or after Phase 3 — it's independent except that Phase 5 requires it.

### Phase 5: StaticFileHandler + MIME detection

**Build:** `src/server/static_files.rs` with `StaticFileHandler` and the `mime_for_extension()` helper.

**Depends on:** Phase 1 (`Response`), Phase 2 (`Handler` trait), Phase 4 (`Url::decoded_path()`).

**Integration:** Register `StaticFileHandler` with the router for `GET /*`. Remove hardcoded file paths from `handle_connection()`. The server now serves any file under the configured root directory.

### Phase 6: Path traversal protection

**Build:** Implement `canonicalize()` check inside `StaticFileHandler::handle()`. Add root normalization in `StaticFileHandler::new()`.

**Depends on:** Phase 5 (`StaticFileHandler`).

**Why last in this group:** The handler is already functional without it, but it must not be deployed without it. This is a security-critical correctness step, not an optimization.

---

## Design Decisions and Rationale

**No generics on Router.** `Box<dyn Handler>` trades a small runtime dispatch overhead for simpler code. For a static file server running on a thread pool, the overhead is negligible versus the disk I/O cost of reading files.

**Vec for routes, not HashMap.** HTTP routing is almost never a hot path relative to I/O. A linear scan of a small route table (typically < 20 entries) is simpler to understand and avoids the complexity of hashing `(method, pattern)` pairs. A HashMap can be introduced later if profiling shows it matters.

**Router holds Arc, not Mutex.** Routes are registered at startup and never modified after `Server::run()` starts. `Arc<Router>` (with no `Mutex`) is sufficient — `Arc` gives shared ownership across threads, and since the router is immutable after construction, `Sync` is automatically satisfied.

**Response body is `Vec<u8>`.** Using `String` for the body would require `fs::read_to_string()` which fails on binary files (images, WASM, etc.). `Vec<u8>` from `fs::read()` works for all content types. Content-Length is the byte count of this Vec.

**`handler.handle()` returns `Result<Response>`, not `Response`.** Handlers are fallible (file not found, I/O errors). Returning `Result` lets the caller decide how to handle failures, and lets `?` propagate errors cleanly within the handler implementation.

**Static root from CLI argument, not code.** The `StaticFileHandler` takes its root from whatever is passed to `Server::new()`. Main passes a CLI argument (or the current directory as a default). This eliminates hardcoded paths and makes the server deployable from any directory.

---

## What Does Not Belong Here

- **Async I/O:** The thread pool model is correct for this project. The `Handler` trait signature (`fn handle(&self, request: &Request) -> Result<Response>`) is synchronous. Migrating to async would require changing the trait signature — that is an explicit out-of-scope decision.
- **Keep-Alive / persistent connections:** HTTP/1.1 specifies persistent connections by default, but implementing them requires reading multiple requests per connection. That is a separate concern from routing; `handle_connection()` can continue closing the connection after one response for now.
- **Request body parsing:** The current `Request::parse()` reads only headers. POST/PUT handlers would need body parsing — that is a future addition to `Request`, not to the router or handler trait.
- **Middleware / interceptors:** A chain-of-responsibility middleware layer can be added later by wrapping `Box<dyn Handler>` in another `Handler` (the decorator pattern). The current design does not need it and should not add it speculatively.

---

*Research completed: 2026-02-28*
