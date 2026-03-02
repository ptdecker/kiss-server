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

### Context struct
- `pub struct Context { pub request: Request, pub response: Response }`
- `Request` and `Response` are public fields on Context — handlers read from `ctx.request` and write to `ctx.response`
- `Request` visibility must be widened from `pub(super)` to at least `pub(crate)` so handlers outside `server::mod` can read it
- Context is constructed per-request inside `handle_connection` and passed `&mut` to the matched handler

### Router
- `pub struct Router { routes: Vec<(String, String, Box<dyn Handler>)> }` — (method, path, handler)
- Registration API: `router.add(method: &str, path: &str, handler: impl Handler + 'static)`
- Dispatch: iterate routes in registration order; first entry where method and path both match (exact string equality) wins
- Fallback: if no route matches, dispatch to a built-in `NotFoundHandler` that writes a 404 response — no need to register it explicitly

### Path matching style
- Exact string equality only for Phase 3 (e.g. `"/"` matches `"/"` only)
- Prefix/glob/wildcard matching deferred to a future phase when static file serving needs it

### Router ownership and Server integration
- `Server` owns an `Arc<Router>` — set at construction time: `Server::new(addr)?.with_router(router).run()`
- `handle_connection` receives `Arc<Router>` as a parameter (or captured from the closure in `pool.execute`)
- The existing `send_error_response` helper remains for pre-routing errors (400, 431); the Router handles post-routing errors (404, 500)

### Error handling
- If a handler returns `Err(_)`: call `send_error_response(stream, 500, "Internal Server Error", ...)` and log the error — do not crash the worker
- The `handle_connection` function wraps the handler call and intercepts the error path

### Claude's Discretion
- Exact field layout of `Context` beyond `request` and `response` (e.g. whether to add a `params` map now or later)
- Whether `Router` derives `Debug` or other traits
- How `NotFoundHandler` is implemented (inline match arm vs dedicated struct)
- Error message body content for 500 responses

</decisions>

<specifics>
## Specific Ideas

- ROUT-02 explicitly requires the no-return-value-reconstruction pattern: handler mutates `ctx.response` in place via `ctx.response = Response::new(200, "OK").header(...).body(...)` — no returning a new Response from the handler
- The `Context` pattern mirrors how production frameworks (Actix, Axum) thread state through handlers, but without macros or async

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `Response` builder (`src/server/response.rs`): handlers build responses with `Response::new(status, reason).header(k,v).body(bytes)` — already solid, no changes needed
- `Request` struct (`src/server/request.rs`): holds `method: RequestMethod` and `target: Url` — needs `pub(crate)` or `pub` visibility for handlers to read it
- `send_error_response` (`src/server/mod.rs`): reusable for the 500 path inside `handle_connection`
- `DateTime::now().to_imf_fixdate()` pattern already established for adding `Date` header

### Established Patterns
- Value-chaining builder for `Response` — handlers should follow this same pattern when constructing their response
- `pub type Result<T> = std::result::Result<T, Error>` in `server/error.rs` — handler trait should use this same Result type
- `Arc<Mutex<...>>` / `Arc` already used in thread pool — `Arc<Router>` is idiomatic here

### Integration Points
- `Server::run()` → `pool.execute(|| handle_connection(stream, Arc::clone(&router)))` — Router reference threaded through pool closure
- `handle_connection` gets refactored: after parsing `Request`, construct `Context { request, response: Response::new(200, "OK") }`, call `router.dispatch(&mut ctx)`, then write `ctx.response` to stream
- The hard-coded `GET /` → "OK" response in `handle_connection` is replaced entirely by the Router dispatch

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
