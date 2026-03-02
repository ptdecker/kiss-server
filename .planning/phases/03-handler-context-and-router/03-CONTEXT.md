# Phase 3: Handler, Context, and Router - Context

**Gathered:** 2026-03-02
**Status:** Ready for planning

<domain>
## Phase Boundary

Define the `Context`, `Handler`, and `Router` abstractions and wire them into the server. Requests are dispatched through the Router to typed Handlers via a shared Context struct. Unmatched routes return 404 (via a built-in `NotFoundHandler` fallback). Handler errors return 500. No middleware, no prefix matching, no keep-alive — those are later phases.

</domain>

<decisions>
## Implementation Decisions

### Handler trait
- `pub trait Handler: Send + Sync { fn handle(&self, ctx: &mut Context) -> Result<()>; }`
- `Send + Sync` required because handlers are shared across worker threads via `Arc`
- Returns `Result<()>` — an `Err` from a handler causes the server to send 500 and log the error
- Handlers are structs that implement the trait (e.g. `struct RootHandler; impl Handler for RootHandler`)

### Handler location
- Handler structs live in a new `src/handlers/` module (e.g. `src/handlers/mod.rs`)
- Not inline in `main.rs` — even for Phase 3's single `RootHandler`
- `main.rs` imports and wires: `use handlers::RootHandler; router.add("GET", "/", RootHandler)?;`

### Context struct
- `pub struct Context { pub request: Request, pub response: Response }`
- `Request` and `Response` are public fields — handlers read `ctx.request` and write `ctx.response`
- Context is constructed per-request inside `handle_connection` with a pre-populated `Response::new(200, "OK")`
- Handler overwrites `ctx.response` to set the actual status, headers, and body

### Router
- `pub struct Router { routes: Vec<(RequestMethod, String, Box<dyn Handler>)> }` — (method enum, path, handler)
- Registration API: `pub fn add(&mut self, method: &str, path: &str, handler: impl Handler + 'static) -> Result<()>`
  - Converts `method: &str` to `RequestMethod` at registration time via `RequestMethod::try_from()`
  - Returns `Err` on invalid method string (e.g. `router.add("FOOBAR", "/", h)` → `Err`)
- Dispatch: iterate routes in registration order; first entry where method and path both match wins
  - Method comparison: `RequestMethod` enum equality
  - Path comparison: exact string equality
- Fallback: if no route matches, dispatch to a built-in `NotFoundHandler` that writes a 404 response — no explicit registration required

### Path matching style
- Exact string equality only for Phase 3 (e.g. `"/"` matches `"/"` only)
- Prefix/glob/wildcard matching deferred to a future phase when static file serving needs it

### Router ownership and Server integration
- `Server` owns an `Arc<Router>` — user passes a plain `Router`, Server wraps it in `Arc` internally
- Builder pattern: `Server::new(addr)?.with_router(router).run()`
- If `with_router()` is not called, Server defaults to an empty `Router` — all requests hit `NotFoundHandler` (404)
- `handle_connection` receives `Arc<Router>` via the pool closure

### Error handling
- If a handler returns `Err(_)`: call `send_error_response(stream, 500, "Internal Server Error", ...)` and log the error — do not crash the worker
- The `handle_connection` function wraps the handler call and intercepts the error path

### Public API surface
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

</decisions>

<specifics>
## Specific Ideas

- ROUT-02 explicitly requires the no-return-value-reconstruction pattern: handler mutates `ctx.response` in place via `ctx.response = Response::new(200, "OK").header(...).body(...)` — no returning a new Response from the handler
- The `Context` pattern mirrors how production frameworks (Actix, Axum) thread state through handlers, but without macros or async
- `RootHandler` (GET /) returns 200 OK with a plain text body — proves the full dispatch pipeline works end-to-end

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `Response` builder (`src/server/response.rs`): handlers build responses with `Response::new(status, reason).header(k,v).body(bytes)` — needs `pub` visibility added
- `Request` struct (`src/server/request.rs`): holds `method: RequestMethod` and `target: Url` — needs `pub` on the struct and both fields
- `RequestMethod` enum (`src/server/request.rs`): full method set, `TryFrom<&str>` impl already exists — needs `pub` added; Router calls this at registration time
- `send_error_response` (`src/server/mod.rs`): reusable for the 500 path inside `handle_connection`
- `DateTime::now().to_imf_fixdate()` pattern already established for adding `Date` header

### Established Patterns
- Value-chaining builder for `Response` — handlers follow this same pattern when constructing their response
- `pub type Result<T> = std::result::Result<T, Error>` in `server/error.rs` — handler trait uses this same Result type
- `Arc` already used in thread pool — `Arc<Router>` is idiomatic here
- `pub use` re-exports at server module root — new types follow same pattern as existing `Error` and `Result` re-exports

### Integration Points
- `Server::run()` → `pool.execute(|| handle_connection(stream, Arc::clone(&router)))` — Router reference threaded through pool closure
- `handle_connection` refactored: after parsing `Request`, construct `Context { request, response: Response::new(200, "OK") }`, call `router.dispatch(&mut ctx)`, then write `ctx.response` to stream
- The hard-coded `GET /` → "OK" response in `handle_connection` is replaced entirely by Router dispatch

</code_context>

<deferred>
## Deferred Ideas

- Middleware chain (ROUT-05, ROUT-06) — explicitly out of scope for Phase 3; deferred to a future phase
- Prefix/wildcard path matching — needed for static file serving but deferred to Phase 4/5
- Method-helper shorthands (`router.get()`, `router.post()`) — deferred; `router.add(method, path, handler)` is sufficient for now

</deferred>

---

*Phase: 03-handler-context-and-router*
*Context gathered: 2026-03-02*
