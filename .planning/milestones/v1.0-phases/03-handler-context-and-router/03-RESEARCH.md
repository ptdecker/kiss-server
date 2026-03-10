# Phase 3: Handler, Context, and Router - Research

**Researched:** 2026-03-02
**Domain:** Rust trait objects, module visibility, Arc-sharing, in-place mutation patterns
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

#### Handler trait
- `pub trait Handler: Send + Sync { fn handle(&self, ctx: &mut Context) -> Result<()>; }`
- `Send + Sync` required because handlers are shared across worker threads via `Arc`
- Returns `Result<()>` — an `Err` from a handler causes the server to send 500 and log the error
- Handlers are structs that implement the trait (e.g. `struct RootHandler; impl Handler for RootHandler`)

#### Handler location
- Handler structs live in a new `src/handlers/` module (e.g. `src/handlers/mod.rs`)
- Not inline in `main.rs` — even for Phase 3's single `RootHandler`
- `main.rs` imports and wires: `use handlers::RootHandler; router.add("GET", "/", RootHandler)?;`

#### Context struct
- `pub struct Context { pub request: Request, pub response: Response }`
- `Request` and `Response` are public fields — handlers read `ctx.request` and write `ctx.response`
- Context is constructed per-request inside `handle_connection` with a pre-populated `Response::new(200, "OK")`
- Handler overwrites `ctx.response` to set the actual status, headers, and body

#### Router
- `pub struct Router { routes: Vec<(RequestMethod, String, Box<dyn Handler>)> }` — (method enum, path, handler)
- Registration API: `pub fn add(&mut self, method: &str, path: &str, handler: impl Handler + 'static) -> Result<()>`
  - Converts `method: &str` to `RequestMethod` at registration time via `RequestMethod::try_from()`
  - Returns `Err` on invalid method string (e.g. `router.add("FOOBAR", "/", h)` → `Err`)
- Dispatch: iterate routes in registration order; first entry where method and path both match wins
  - Method comparison: `RequestMethod` enum equality
  - Path comparison: exact string equality
- Fallback: if no route matches, dispatch to a built-in `NotFoundHandler` that writes a 404 response — no explicit registration required

#### Path matching style
- Exact string equality only for Phase 3
- Prefix/glob/wildcard matching deferred to a future phase when static file serving needs it

#### Router ownership and Server integration
- `Server` owns an `Arc<Router>` — user passes a plain `Router`, Server wraps it in `Arc` internally
- Builder pattern: `Server::new(addr)?.with_router(router).run()`
- If `with_router()` is not called, Server defaults to an empty `Router` — all requests hit `NotFoundHandler` (404)
- `handle_connection` receives `Arc<Router>` via the pool closure

#### Error handling
- If a handler returns `Err(_)`: call `send_error_response(stream, 500, "Internal Server Error", ...)` and log the error — do not crash the worker
- The `handle_connection` function wraps the handler call and intercepts the error path

#### Public API surface
- `pub trait Handler`, `pub struct Context`, `pub struct Router`, `pub struct Response` — all accessible from `src/handlers/`
- `pub enum RequestMethod` — handlers can pattern-match on `ctx.request.method`
- `pub struct Request { pub method: RequestMethod, pub target: Url }` — direct field access in handlers
- All new and promoted types re-exported from server module root:
  `use server::{Handler, Context, Router, Response, RequestMethod};`

### Claude's Discretion
- Exact field layout of `Context` beyond `request` and `response`
- Whether `Router` derives `Debug` or other traits
- How `NotFoundHandler` is implemented (inline match arm vs dedicated struct)
- Error message body content for 500 responses

### Deferred Ideas (OUT OF SCOPE)
- Middleware chain (ROUT-05, ROUT-06) — explicitly out of scope for Phase 3; deferred to a future phase
- Prefix/wildcard path matching — needed for static file serving but deferred to Phase 4/5
- Method-helper shorthands (`router.get()`, `router.post()`) — deferred; `router.add(method, path, handler)` is sufficient for now
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| ROUT-01 | Router dispatches requests to handlers registered in registration order | Vec iterated in insertion order; first method+path match wins |
| ROUT-02 | Handler trait takes `&mut Context` and mutates the response in place (no return value reconstruction) | Handler returns `Result<()>`; ctx.response assigned in place |
| ROUT-03 | Unmatched routes fall through to a NotFoundHandler fallback | Built-in fallback after Vec exhaustion; NotFoundHandler writes 404 |
| ROUT-04 | Context struct holds Request and Response as mutable shared pipeline state | `pub struct Context { pub request: Request, pub response: Response }` |
| HTTP-07 | Server responds 404 Not Found when no route matches | NotFoundHandler sets status 404 on ctx.response; handle_connection writes it |
| HTTP-08 | Server responds 500 Internal Server Error on unhandled errors | handle_connection catches Err from dispatch; calls send_error_response(500); logs |
</phase_requirements>

---

## Summary

Phase 3 is a pure Rust design phase — no new crate dependencies. The entire work is defining three new types (`Handler` trait, `Context` struct, `Router` struct), promoting visibility on existing types (`RequestMethod`, `Request`, `Response`), creating a `src/handlers/` module with `RootHandler`, and refactoring `handle_connection` and `Server` to wire it all together. All 24 existing tests pass and must continue to pass.

The existing codebase already has all the raw materials: `Response` is `pub` with a complete builder API; `Request` and `RequestMethod` are `pub(super)` and need promotion to `pub`; `send_error_response` exists in `server/mod.rs` and is directly reusable for the 500 error path; `Arc` is already imported and used in `server/mod.rs` for the thread pool. The `Url` struct in `src/url/mod.rs` has no `.path()` accessor — only `Display` via `to_string()` — so router path comparison uses `ctx.request.target.to_string()`.

The central design challenge is ownership: `Router` must be `Arc`-wrapped so multiple threads can share it. Handlers inside `Router` are `Box<dyn Handler>` with `Send + Sync` bounds, ensuring they safely cross thread boundaries. The `handle_connection` function signature changes from `fn handle_connection(stream: TcpStream)` to `fn handle_connection(stream: TcpStream, router: Arc<Router>)`, and `Server::run()` must capture `Arc::clone(&self.router)` into each pool closure.

**Primary recommendation:** Implement in four atomic steps: (1) promote visibility and add pub re-exports, (2) define Handler/Context/Router in `server/`, (3) create `src/handlers/mod.rs` with RootHandler, (4) refactor handle_connection and Server to use Arc<Router>.

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Rust std (trait objects) | std | `Box<dyn Handler>` for heterogeneous handler storage | Only std mechanism for runtime-dispatch polymorphism without enums |
| Rust std (Arc) | std | `Arc<Router>` for shared ownership across threads | Already in use in this codebase for the thread pool receiver |
| log crate | 0.4.20 | Logging handler errors on 500 path | Already in use — no new dependency |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| Rust std (PartialEq derive) | std | `RequestMethod` comparison in router dispatch | Required — `RequestMethod` currently missing `PartialEq` derive |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `Box<dyn Handler>` | Generic `<H: Handler>` per router | Generic loses heterogeneous storage; cannot hold different handler types in same Vec |
| `Arc<Router>` | Clone Router per thread | Impossible — `Box<dyn Handler>` is not `Clone`; Arc is the only option |
| Separate `context.rs` / `handler.rs` / `router.rs` | All in `server/mod.rs` | Files preferred for clarity; either compiles; locked decision leaves file layout to discretion |

**Installation:** No new dependencies required.

---

## Architecture Patterns

### Recommended Project Structure

```
src/
├── handlers/
│   └── mod.rs          # pub struct RootHandler; impl Handler for RootHandler
├── server/
│   ├── context.rs      # pub struct Context { pub request: Request, pub response: Response }
│   ├── error.rs        # existing — no changes
│   ├── handler.rs      # pub trait Handler: Send + Sync { fn handle(...) -> Result<()>; }
│   ├── mod.rs          # Server (add router field + with_router), handle_connection (refactored)
│   ├── pool.rs         # existing — no changes
│   ├── request.rs      # visibility: pub enum RequestMethod, pub struct Request, pub fields
│   ├── response.rs     # existing pub struct Response — no changes
│   ├── router.rs       # pub struct Router; struct NotFoundHandler (private)
│   └── worker.rs       # existing — no changes
└── main.rs             # wires: Router + RootHandler, Server::new().with_router(router).run()
```

### Pattern 1: Trait Object Handler Storage

**What:** Handlers are stored as `Box<dyn Handler>` inside the Router's Vec. This allows the Router to hold handlers of different concrete types without generics.

**When to use:** Any collection that must hold heterogeneous values sharing a common interface.

**Example:**
```rust
// Source: Rust Reference — Trait Objects
// https://doc.rust-lang.org/reference/types/trait-object.html

pub trait Handler: Send + Sync {
    fn handle(&self, ctx: &mut Context) -> Result<()>;
}

pub struct Router {
    routes: Vec<(RequestMethod, String, Box<dyn Handler>)>,
}

impl Router {
    pub fn new() -> Self {
        Router { routes: Vec::new() }
    }

    pub fn add(
        &mut self,
        method: &str,
        path: &str,
        handler: impl Handler + 'static,
    ) -> Result<()> {
        let method = RequestMethod::try_from(method)?;
        self.routes.push((method, path.to_string(), Box::new(handler)));
        Ok(())
    }
}
```

### Pattern 2: Arc<Router> Thread-Sharing

**What:** Router is `Arc`-wrapped so each thread pool closure gets a cheap reference count increment without copying handler data.

**When to use:** Shared-read, no-mutation state that must cross thread boundaries.

**Example:**
```rust
// Source: Rust std — Arc
// https://doc.rust-lang.org/std/sync/struct.Arc.html

pub struct Server {
    addr: String,
    listener: TcpListener,
    pool: ThreadPool,
    router: Arc<Router>,
}

impl Server {
    pub fn new(addr: impl Into<String>) -> Result<Server> {
        let addr = addr.into();
        Ok(Server {
            addr: addr.clone(),
            listener: TcpListener::bind(&addr)?,
            pool: ThreadPool::build(DEFAULT_POOL_SIZE)?,
            router: Arc::new(Router::new()),  // default empty router
        })
    }

    pub fn with_router(mut self, router: Router) -> Self {
        self.router = Arc::new(router);
        self
    }

    pub fn run(&self) -> Result<()> {
        info!("Listening for connections on {}", &self.addr);
        for stream_result in self.listener.incoming() {
            let router = Arc::clone(&self.router);  // clone BEFORE move closure
            self.pool.execute(move || match stream_result {
                Ok(stream) => {
                    handle_connection(stream, router)
                        .unwrap_or_else(|e| warn!("handle_connection: {}", e))
                }
                Err(e) => warn!("thread: {}", e),
            })?;
        }
        info!("Shutting down");
        Ok(())
    }
}
```

### Pattern 3: Context In-Place Mutation (ROUT-02)

**What:** Handlers receive `&mut Context` and mutate `ctx.response` directly. Handler returns `Result<()>`, not `Response`. This satisfies ROUT-02 and enables future middleware without reconstruction overhead.

**When to use:** Middleware-composable pipelines where multiple stages write to the same response.

**Example:**
```rust
// src/handlers/mod.rs
use crate::server::{Context, Handler, Response};
use crate::server::Result;

pub struct RootHandler;

impl Handler for RootHandler {
    fn handle(&self, ctx: &mut Context) -> Result<()> {
        let body = b"OK".to_vec();
        let content_length = body.len().to_string();
        ctx.response = Response::new(200, "OK")
            .header("Content-Type", "text/plain")
            .header("Content-Length", &content_length)
            .header("Connection", "close")
            .body(body);
        Ok(())
    }
}
```

### Pattern 4: Router Dispatch with NotFoundHandler Fallback

**What:** Router iterates routes in registration order. On first method+path match, calls that handler. If no match, calls a private `NotFoundHandler`.

**Path comparison:** `Url` has no `.path()` method — use `ctx.request.target.to_string()` which calls the `Display` impl that formats as the raw path string (e.g. `"/"`).

**Example:**
```rust
// src/server/router.rs
impl Router {
    pub fn dispatch(&self, ctx: &mut Context) -> Result<()> {
        let method = &ctx.request.method;
        let path = ctx.request.target.to_string();  // Display impl gives raw_path
        for (route_method, route_path, handler) in &self.routes {
            if route_method == method && route_path.as_str() == path.as_str() {
                return handler.handle(ctx);
            }
        }
        // No match — built-in fallback
        NotFoundHandler.handle(ctx)
    }
}

struct NotFoundHandler;

impl Handler for NotFoundHandler {
    fn handle(&self, ctx: &mut Context) -> Result<()> {
        let body = b"Not Found".to_vec();
        let content_length = body.len().to_string();
        ctx.response = Response::new(404, "Not Found")
            .header("Content-Type", "text/plain")
            .header("Content-Length", &content_length)
            .header("Connection", "close")
            .body(body);
        Ok(())
    }
}
```

### Pattern 5: Handle Connection Refactor

**What:** `handle_connection` gains `Arc<Router>` parameter, constructs `Context` after request parse, calls `router.dispatch(&mut ctx)`, intercepts `Err` for 500 response, injects Date header, writes `ctx.response`.

**Date header:** The old `handle_connection` injected Date inline. After refactor, `handle_connection` must inject Date after dispatch returns `Ok(())` and before writing — otherwise HTTP-03 breaks. Since `Response::write_to` consumes self (builder chain), a `pub fn add_header(&mut self, name: &str, value: &str)` mutating method must be added to `Response`, or the Date injection must occur differently. See Open Questions for final resolution.

**Example:**
```rust
// src/server/mod.rs
fn handle_connection(mut stream: TcpStream, router: Arc<Router>) -> Result<()> {
    info!("handling a connection");

    // ... existing BufReader block for header collection (unchanged) ...

    let request = match Request::parse(&http_request) {
        Ok(r) => r,
        Err(e) => {
            send_error_response(&mut stream, 400, "Bad Request", &e.to_string());
            return Err(e);
        }
    };

    info!("{}", http_request[0]);

    // Construct Context with a default 200 response (handler overwrites)
    let mut ctx = Context {
        request,
        response: Response::new(200, "OK"),
    };

    // Dispatch — handler mutates ctx.response in place
    if let Err(e) = router.dispatch(&mut ctx) {
        warn!("handler error: {}", e);
        send_error_response(&mut stream, 500, "Internal Server Error", "Internal Server Error");
        return Err(e);
    }

    // Write handler-built response to stream
    ctx.response.write_to(&mut stream)?;
    Ok(())
}
```

### Pattern 6: Visibility Promotion (Critical Prerequisite)

**What:** `Request`, `RequestMethod` and their fields must be `pub` (not `pub(super)`) so the `handlers` module outside `server` can use them via re-exports.

**Changes needed:**

```rust
// src/server/request.rs: promote visibility
pub const MAX_HEADER_LINES: usize = 100;  // or keep pub(super) since handlers won't use it

#[derive(Debug, PartialEq, Copy, Clone)]  // ADD PartialEq — required for router dispatch
pub enum RequestMethod { ... }             // was pub(super)

#[derive(Debug, Clone)]
pub struct Request {
    pub method: RequestMethod,  // was pub(super)
    pub target: Url,            // was pub(super)
}

// src/server/mod.rs: add to re-exports
pub use context::Context;
pub use handler::Handler;
pub use router::Router;
pub use request::{Request, RequestMethod};
// Response is already pub — add re-export:
pub use response::Response;
```

### Anti-Patterns to Avoid

- **Returning Response from handler:** Handler must mutate `ctx.response`, not return a new `Response`. ROUT-02 explicitly prohibits reconstruction.
- **Registering NotFoundHandler explicitly:** The fallback is internal to Router — private struct, never in `routes` Vec.
- **Cloning Router instead of Arc-wrapping:** `Router` cannot implement `Clone` because `Box<dyn Handler>` is not `Clone`. Must use `Arc<Router>`.
- **Making Handler generic on response type:** Breaks object safety. Handler trait must use concrete `Result<()>` to be storable as `Box<dyn Handler>`.
- **Omitting PartialEq on RequestMethod:** Router dispatch requires `==` comparison. Must add `#[derive(PartialEq)]`.
- **Calling `.path()` on Url:** `Url` has no `.path()` method — use `.to_string()` (Display impl).

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Trait object dispatch | Custom vtable or enum dispatch | `Box<dyn Handler>` — Rust built-in vtable | Rust handles dispatch, size-erasure, and drop |
| Thread-safe shared ownership | Custom reference counting | `Arc<Router>` | Arc is the standard, already in use in this codebase |
| Method string parsing | Custom string matching | `RequestMethod::try_from(&str)` in `request.rs` | Already built and tested — reuse at registration time |
| Error propagation | New error types | Existing `server::Result<T>` and `server::Error` | Error enum already covers all cases |

**Key insight:** All infrastructure exists. The work is wiring, visibility promotion, and defining three new types — not building new infrastructure.

---

## Common Pitfalls

### Pitfall 1: Object Safety Violation

**What goes wrong:** Compiler rejects `Box<dyn Handler>` with "the trait `Handler` cannot be made into an object."

**Why it happens:** A trait is not object-safe if any method uses `Self` in return position, has generic type parameters, or returns `impl Trait`.

**How to avoid:** Keep `Handler` to exactly one method: `fn handle(&self, ctx: &mut Context) -> Result<()>`. No generics, no `Self` return, no associated types. `Send + Sync` supertraits are object-safe.

**Warning signs:** Compiler error "E0038: the trait ... cannot be made into an object."

### Pitfall 2: Arc<Router> Moved Into Closure Instead of Cloned

**What goes wrong:** `pool.execute(move || handle_connection(stream, self.router))` compiles once, then fails on the second loop iteration.

**Why it happens:** `move` closures take ownership. The first closure consumes `self.router`; subsequent iterations have nothing to move.

**How to avoid:** Clone `Arc` before the closure, assign to local, capture local:
```rust
let router = Arc::clone(&self.router);  // clone BEFORE move closure
self.pool.execute(move || handle_connection(stream, router))?;
```

**Warning signs:** Compiler error "use of moved value: `self.router`."

### Pitfall 3: RequestMethod Missing PartialEq

**What goes wrong:** `route_method == method` in router dispatch fails to compile.

**Why it happens:** `RequestMethod` currently derives `Debug, Copy, Clone` but not `PartialEq`. The `==` operator requires `PartialEq`.

**How to avoid:** Add `PartialEq` to the derive on `RequestMethod` in `request.rs`:
```rust
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum RequestMethod { ... }
```

**Warning signs:** Compiler error "binary operation `==` cannot be applied to type `RequestMethod`."

### Pitfall 4: Response write_to Consumes Self

**What goes wrong:** Trying to add headers to `ctx.response` after dispatch, or reading from it after writing, fails with "use of moved value."

**Why it happens:** `Response::write_to(self, ...)` takes ownership (consuming). This prevents double-send but means you cannot call it and then access the response again.

**How to avoid:** Any Date header injection (see Pitfall 7) must occur before calling `write_to`. If a mutable `add_header` method is added to `Response`, use it before the write call.

**Warning signs:** Compiler error "use of moved value: `ctx.response`."

### Pitfall 5: handle_connection Signature Change Breaks Tests

**What goes wrong:** Changing signature to `handle_connection(stream, router: Arc<Router>)` breaks the test helper `spawn_handle_connection_test` in `server/mod.rs`.

**Why it happens:** Tests call `handle_connection` directly with the old 1-argument signature.

**How to avoid:** Update `spawn_handle_connection_test` to accept `Arc<Router>` and pass it through. Existing `get_root_returns_200` test must pass a router with `RootHandler` registered at `GET /` or it will receive 404 instead of 200.

**Warning signs:** Compiler error "this function takes 1 argument but 2 arguments were supplied."

### Pitfall 6: Visibility Gap — handlers Module Cannot See server Types

**What goes wrong:** `src/handlers/mod.rs` gets compile error accessing `Request`, `RequestMethod`, or `Context` from `crate::server`.

**Why it happens:** `pub(super)` on `Request`/`RequestMethod` limits visibility to the `server` module. `handlers` is outside `server`.

**How to avoid:** Promote to `pub` in `request.rs` and add `pub use request::{Request, RequestMethod}` to `server/mod.rs`. Do this before writing any handler code.

**Warning signs:** Compiler error "struct `Request` is pub(super) and not accessible here."

### Pitfall 7: Date Header Lost After Refactor (HTTP-03 Regression)

**What goes wrong:** After refactor, responses no longer include a `Date` header, breaking HTTP-03.

**Why it happens:** The old `handle_connection` built the response and injected Date inline. After refactor, `ctx.response` is handler-built — handlers don't know about the Date requirement, and `handle_connection` no longer injects it.

**How to avoid (two options):**

Option A (recommended): Add a mutating `pub fn add_header(&mut self, name: &str, value: &str)` to `Response` so `handle_connection` can inject Date after dispatch:
```rust
// After router.dispatch(&mut ctx) returns Ok(())
if let Ok(dt) = DateTime::now() {
    ctx.response.add_header("Date", &dt.to_imf_fixdate());
}
ctx.response.write_to(&mut stream)?;
```

Option B: Handlers each add Date via the builder chain. This is repetitive, error-prone, and violates DRY.

Option A keeps HTTP-03 compliance in one place. The `add_header` method on `Response` is a small addition: `pub fn add_header(&mut self, name: &str, value: &str) { self.headers.push((name.to_string(), value.to_string())); }`.

**Warning signs:** `get_root_response_has_required_headers` test fails (asserts `response.contains("Date:")`) after refactor.

---

## Code Examples

Verified patterns from the existing codebase:

### Existing RequestMethod::try_from — reuse at router registration

```rust
// Source: src/server/request.rs (verified 2026-03-02)
impl TryFrom<&str> for RequestMethod {
    type Error = Error;
    fn try_from(value: &str) -> Result<Self> {
        match value {
            "GET" => Ok(RequestMethod::Get),
            "HEAD" => Ok(RequestMethod::Head),
            // ... all 8 methods ...
            _ => Err(Error::InvalidRequest(format!("invalid method: {value}"))),
        }
    }
}

// Router.add() reuses this at registration time:
let method = RequestMethod::try_from(method_str)?;  // Err propagates to caller
self.routes.push((method, path.to_string(), Box::new(handler)));
```

### Existing send_error_response — reuse for 500 path

```rust
// Source: src/server/mod.rs (verified 2026-03-02)
// Already handles Date header and best-effort write — use as-is for 500 path
fn send_error_response(stream: &mut TcpStream, status: u16, reason: &'static str, message: &str) {
    // ... builds Response with Date header, best-effort write_to ...
}

// Usage in handle_connection after dispatch error:
if let Err(e) = router.dispatch(&mut ctx) {
    warn!("handler error: {}", e);
    send_error_response(&mut stream, 500, "Internal Server Error", "Internal Server Error");
    return Err(e);
}
```

### Existing Arc pattern — extend to Router

```rust
// Source: src/server/pool.rs (verified 2026-03-02)
// Thread pool already uses Arc<Mutex<Receiver<Job>>>:
let receiver = Arc::new(Mutex::new(receiver));
workers.push(Worker::new(id, Arc::clone(&receiver))?);

// Router follows same pattern (without Mutex — Router is read-only after construction):
pub fn with_router(mut self, router: Router) -> Self {
    self.router = Arc::new(router);
    self
}

// In run() loop:
let router = Arc::clone(&self.router);
self.pool.execute(move || handle_connection(stream, router))?;
```

### Url path extraction — use Display, not .path()

```rust
// Source: src/url/mod.rs (verified 2026-03-02)
// Url struct: only field is private raw_path: String
// Only public interfaces: Display (formats as raw_path) and From<&str>
// There is NO .path() method.

// Correct — use to_string() for path comparison in router:
let path = ctx.request.target.to_string();  // e.g. "/" or "/about"
if route_path.as_str() == path.as_str() { ... }

// Wrong — does not compile:
// let path = ctx.request.target.path();  // no such method
```

### Server builder pattern (new wiring in main.rs)

```rust
// Source: CONTEXT.md locked decision
use handlers::RootHandler;
use server::{Router, Server};

fn main() -> Result<()> {
    SimpleLogger::init()?;
    let mut router = Router::new();
    router.add("GET", "/", RootHandler)?;
    Server::new(DEFAULT_ADDR)?.with_router(router).run()?;
    Ok(())
}
```

### Test helper update (preserves existing test coverage)

```rust
// src/server/mod.rs tests — update spawn_handle_connection_test signature
fn spawn_handle_connection_test(send_bytes: &'static [u8], router: Arc<Router>) -> Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let client_thread = thread::spawn(move || { /* ... unchanged ... */ });
    let (stream, _) = listener.accept().unwrap();
    let result = handle_connection(stream, router);  // pass router
    client_thread.join().unwrap();
    result
}

// Existing tests pass router:
#[test]
fn get_root_returns_200() {
    let mut router = Router::new();
    router.add("GET", "/", RootHandler).unwrap();
    let result = spawn_handle_connection_test(
        b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n",
        Arc::new(router),
    );
    assert!(result.is_ok());
}

#[test]
fn invalid_utf8_returns_err_not_panic() {
    // Empty router is fine — request fails before dispatch
    let result = spawn_handle_connection_test(
        b"\xFF\xFE binary garbage\r\n\r\n",
        Arc::new(Router::new()),
    );
    assert!(result.is_err());
}
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `handle_connection(stream)` hardcoded 200 response | `handle_connection(stream, Arc<Router>)` dispatches dynamically | Phase 3 | All responses routed through handlers |
| `pub(super)` on Request/RequestMethod | `pub` with server root re-exports | Phase 3 | handlers module outside server can use request types |
| No handler abstraction | `pub trait Handler: Send + Sync` | Phase 3 | Pluggable handlers; enables future middleware (ROUT-05/06) |
| Response built inline in handle_connection | Response built in handler, written in handle_connection | Phase 3 | Clean separation of concerns |

**Deprecated/outdated:**
- Hardcoded `GET /` → `b"OK"` body block in `handle_connection`: replaced by Router dispatch through `RootHandler`

---

## Open Questions

1. **Date header injection — how to add after dispatch without double-build**

   - What we know: `Response::write_to` is consuming (takes `self`). The builder chain `.header()` is also consuming. There is no `&mut self` mutation method on `Response` currently.
   - What's unclear: Does handle_connection add Date before or after dispatch? Does Response need a new mutation method?
   - Recommendation: Add `pub fn add_header(&mut self, name: &str, value: &str)` to `Response`. This is a one-line addition to `response.rs`. After `router.dispatch(&mut ctx)` returns `Ok(())`, call `ctx.response.add_header("Date", &dt.to_imf_fixdate())`, then `ctx.response.write_to(&mut stream)?`. This preserves HTTP-03 compliance without requiring every handler to manage Date.

2. **Url path extraction — RESOLVED**

   - Confirmed: `src/url/mod.rs` exposes only `Display` and `From<&str>`. No `.path()` method exists.
   - Solution: Router dispatch uses `ctx.request.target.to_string()` for path string comparison.
   - Future phases (PATH-01 percent-decode, PATH-02 `..` rejection) will require adding methods to `Url`. Phase 3 does not need to address this.

3. **Test update scope for `get_root_response_has_required_headers`**

   - What we know: This test reads the full HTTP response and asserts Date header presence. After refactor, Date injection location changes.
   - What's unclear: The test asserts `response.contains("Date:")` — this will fail unless Date is injected somewhere in the happy path.
   - Recommendation: The Date injection in `handle_connection` (post-dispatch, pre-write) via the new `add_header` method resolves this. The test continues to pass unchanged.

---

## Sources

### Primary (HIGH confidence)
- Rust Reference: Trait Objects — https://doc.rust-lang.org/reference/types/trait-object.html
- Rust Reference: Visibility and Privacy — https://doc.rust-lang.org/reference/visibility-and-privacy.html
- Rust std: Arc — https://doc.rust-lang.org/std/sync/struct.Arc.html
- Existing codebase — read and verified 2026-03-02:
  - `src/server/mod.rs` (Server, handle_connection, send_error_response, tests)
  - `src/server/request.rs` (RequestMethod, Request, TryFrom impl)
  - `src/server/response.rs` (Response builder, write_to consuming)
  - `src/server/pool.rs` (Arc usage pattern)
  - `src/server/error.rs` (Result type alias)
  - `src/url/mod.rs` (Url — only Display and From<&str>; no .path() method)
  - `src/main.rs` (current wiring)
  - `Cargo.toml` (no new deps needed)

### Secondary (MEDIUM confidence)
- Rust Book Chapter 17: Object-Oriented Programming — https://doc.rust-lang.org/book/ch17-00-oop.html

### Tertiary (LOW confidence)
- None — all claims verified against existing code or official docs

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new dependencies; all patterns verified in existing codebase
- Architecture: HIGH — locked decisions in CONTEXT.md; Rust visibility rules are stable and well-documented
- Pitfalls: HIGH — derived from direct code inspection; `pub(super)` visibility gap and missing `PartialEq` confirmed by reading source; `Url` accessor gap confirmed by reading `src/url/mod.rs`

**Research date:** 2026-03-02
**Valid until:** 2026-04-02 (stable Rust patterns; no fast-moving dependencies)
