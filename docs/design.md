# Architecture

The kiss-server is a from-scratch HTTP/1.1 static file server written in pure Rust. No external
dependencies beyond the `log` crate facade. This document explains the core design patterns and key
decisions.

## Request Lifecycle

Every HTTP request passes through three layers in order:

1. **Middleware chain** — runs before dispatch. Each middleware inspects or mutates the `Context`
   and returns either `Continue` (pass to the next middleware) or `ShortCircuit` (write a response
   and stop). Routes on the public-route exemption list bypass the chain entirely. The built-in
   `AuthMiddleware` checks for an `X-Authenticated-User` header and short-circuits with 401 if it
   is absent or blank. Set `KISS_SKIP_AUTH=1` to disable auth for local development.

2. **Router dispatch** — matches method + path to the first registered handler. Exact routes take
   priority over prefix routes; first registration wins on ties. The router rejects dot-dot
   components and invalid %-sequences with 404 before any handler is called (defense in depth).
   Unmatched requests fall through to the fallback handler.

3. **Handler** — produces the response by writing into `ctx.response`. The `VhostDispatcher`
   fallback dispatches to the per-domain `StaticFileHandler` (or the default handler) based on the
   normalized `Host` header.

## Handler, Context, and Router

The request pipeline is built around three types that work together:

- **Handler trait**: `fn handle(&self, ctx: &mut Context) -> Result<()>` — any struct implementing
  Handler can process requests. In-place mutation of Context avoids future reconstruction overhead.
  Handlers must be `Send + Sync` because they are shared across worker threads via `Arc<Router>`.
  Returning `Err` causes the server to send a 500 response.

- **Context**: Wraps the incoming `Request`, the outgoing `Response`, and optional `AuthClaims` for
  a single HTTP request/response cycle. Constructed per-request in `handle_connection`. Handlers
  read from `ctx.request` and write into `ctx.response` in place. `ctx.auth` carries the
  authenticated user identity set by `AuthMiddleware`.

- **Router**: Maps URL paths to Handler implementations via a registration list. Routes are checked
  in registration order; the first match wins. Uses `set_fallback()` for the vhost dispatcher —
  unmatched routes fall through to file serving. Prefix routes (registered with `add_prefix()`) are
  used for plugins; an exact route on the same path takes priority.

## Middleware

`MiddlewareChain` (`src/server/middleware.rs`) holds an ordered list of `Middleware` trait objects
and a public-route exemption list. It runs before `router.dispatch()`:

- Routes matching the exemption list (exact decoded-path match) skip all middleware.
- Non-exempt routes pass through each middleware in registration order.
- The first `ShortCircuit` stops the chain — subsequent middleware does not run.
- `AuthMiddleware` (`src/server/auth.rs`) validates `X-Authenticated-User` and populates
  `ctx.auth` on success, or writes a 401 response and returns `ShortCircuit` on failure.

## Plugin System

Plugins extend the server with prefix-routed handlers loaded at startup from a TOML config file.

**Plugin SDK** (`kiss-plugin-sdk` crate): defines the shared types plugins are built against —
`Handler`, `KissPlugin`, `Context`, `Request`, `Response`, `AuthClaims`, `PluginConfig`. Plugins
depend only on the SDK crate, not on server internals.

**`KissPlugin` trait**: extends `Handler` with `name() -> &str` and `path_prefix() -> &str`.
`main.rs` maps each configured plugin name to its constructor and registers it on the router under
its declared prefix.

**Bundled plugin**: `kiss-url-shortener` (`kiss-url-shortener` crate) — in-memory URL shortener
at prefix `/s/<code>`. It is the reference implementation.

**`--root` vs `--config` modes** (`src/main.rs:build_dispatcher()`):

- `--config <path>`: loads TOML config; activates vhosts, `default_root`, and `[[plugin]]` blocks.
- `--root <path>`: backward-compatible simple mode; returns an empty plugin list — plugins are not
  available. The reason: plugin names cannot be expressed on the command line without a structured
  config file, and `--root` is intentionally kept simple. The two flags are mutually exclusive.
- If plugins are ever needed without vhost config, the right fix is a minimal config file with only
  `[[plugin]]` blocks, not plugin flags on `--root`.

## Virtual Hosting

`VhostDispatcher` (`src/handlers/vhost.rs`) implements `Handler` and is installed as the router
fallback. On each request it normalizes the `Host` header (lowercase, strips port, strips `www.`
prefix) and looks up the matching `StaticFileHandler` in a domain map. If no match is found it
falls back to the configured `default_handler`, or returns a parked-domain page if neither exists.

## Thread Pool

- Fixed-size thread pool (default 4 workers, configurable via CLI) for concurrent connection
  handling.
- Synchronous blocking I/O — each connection gets a thread from the pool for the full duration of
  handling.
- No async runtime (no tokio). The thread pool is enough for the project's goals and keeps the
  implementation simple.
- Worker threads receive jobs via a shared `mpsc` channel behind a `Mutex<Receiver>`. Each worker
  holds `Arc<Mutex<Receiver<Job>>>` and calls `lock().unwrap_or_else(...)` to recover from mutex
  poison.
- Graceful-ish shutdown: the `Drop` implementation drops the sender (closing the channel), then
  joins all worker threads. `let _ = thread.join()` it avoids panic propagation if a worker
  panicked.

## Static File Serving

- `StaticFileHandler` implements the Handler trait and is installed as the router's fallback.
- Serves files from a configurable root directory (`--root` CLI argument, required at startup).
- Binary-safe reads — files are read as `Vec<u8>` via `fs::read()`, not as strings.
- MIME detection from file extension (10 types: `html`, `css`, `js`, `wasm`, `png`, `jpg`, `jpeg`,
  `gif`, `svg`, `ico`, `txt`; with `application/octet-stream` fallback).
- Directory requests automatically serve `index.html` from within that directory.
- **Path traversal prevention**: `canonicalize()` the candidate path after joining with the root,
  then verify it `starts_with(canonical_root)`. Returns 404 (not 403) on traversal attempts to avoid
  leaking whether a path exists outside the root. Symlinks that escape the root are caught at this
  step.
- The router also rejects dot-dot components and invalid %-sequences before the handler is called,
  providing defense in depth.

## Error Handling

- `Result` propagation throughout — no `unwrap()` on unhappy paths in production code.
- Custom `Error` type wraps `String` for simplicity, with variants for channel errors,
  request-too-large, and I/O errors.
- Malformed or non-UTF-8 HTTP requests return 400 Bad Request (not a panic).
- Request header limit of 100 lines; exceeding returns 431 Request Header Fields Too Large.
- Mutex poison in worker threads handled via `unwrap_or_else` recovery.
- ThreadPool `Drop` uses `let _ = thread.join()` to avoid panic propagation.
- Date header is injected by `handle_connection` after dispatch (cross-cutting concern). If
  `DateTime::now()` fails, the Date header is omitted rather than panicking (never fails in
  practice).

## Key Decisions

| Decision                                                    | Rationale                                                                        | Outcome |
|-------------------------------------------------------------|----------------------------------------------------------------------------------|---------|
| Keep `log` crate                                            | Already in use; minimal facade with zero runtime cost                            | Good    |
| Router-first design                                         | Static files are a route handler, not special-cased logic                        | Good    |
| `--root` / `--config` as mutually exclusive modes           | `--root` stays simple (no config file needed); `--config` unlocks vhosts and plugins | Good |
| Plugins only available under `--config`, not `--root`       | Plugin names require a structured config file; `--root` is intentionally minimal | Good    |
| Plugin SDK as a separate crate (`kiss-plugin-sdk`)          | Plugins depend only on SDK types, not server internals; clear ABI boundary       | Good    |
| Middleware chain before router dispatch                      | Cross-cutting concerns (auth, future rate-limiting) separated from handler logic | Good    |
| Public-route exemption list on `MiddlewareChain`            | `/health`, `/favicon.ico` must bypass auth without special-casing in each middleware | Good |
| `AuthMiddleware` trusts `X-Authenticated-User` header       | Lambda@Edge validates JWTs at the edge; origin trusts the injected header because port 80 is locked to CloudFront IPs | Good |
| `KISS_SKIP_AUTH` env var disables auth for local dev        | No Lambda@Edge in development; env var is explicit and visible (logged at startup) | Good  |
| `VhostDispatcher` as router fallback                        | Virtual hosting is a dispatch concern, not a server-core concern; keeps Router generic | Good |
| No async runtime                                            | Learning project stays simple; thread pool sufficient for goals                  | Good    |
| Howard Hinnant `civil_from_days`                            | O(1) Gregorian arithmetic, replaces iterative year and month loops               | Good    |
| `Result` propagation throughout                             | Eliminate all `unwrap()` on unhappy paths                                        | Good    |
| `Vec<(String,String)>` for headers                          | Preserves insertion order; no hashing overhead for <15 headers                   | Good    |
| `write_to` consumes `Response` (`self`)                     | Prevents double-send; ownership enforces correct use at compile time             | Good    |
| Handler trait `fn handle(&self, ctx: &mut Context)`         | In-place mutation; no reconstruction overhead; enables future middleware         | Good    |
| `add_header` vs `header`                                    | Builder for construction, `add_header` for post-dispatch injection (Date header) | Good    |
| Date header injected by `handle_connection`                 | Cross-cutting concern owned by server layer; handlers don't need to know         | Good    |
| PATH-03 in `StaticFileHandler`, not Router                  | Requires configured root path; deferred correctly                                | Good    |
| `decoded_path()` uses byte-buffer with `hex_char_to_byte()` | Avoids `pct_decode()` multi-byte-per-call complexity                             | Good    |
| 404 on path traversal (not 403)                             | Avoids leaking whether the path exists outside root                              | Good    |
| CloudFront for TLS termination (not server-side)            | Separation of concerns; auto-renewing ACM certs; CDN benefits free               | Good    |
| `Connection: close` on all responses                        | Server handles one request per connection; prevents CLOSE-WAIT pile-up           | Good    |
| 30-second read timeout                                      | Prevents worker exhaustion by stalled or slow clients                            | Good    |
| Security group restricts port 80 to CloudFront prefix list  | Eliminates direct HTTP bypass of TLS/auth; port 22 remains open for SSH admin    | Good    |

## TLS Termination Architecture

HTTPS termination is handled by AWS CloudFront, not by kiss-server itself. The server remains a
pure HTTP/1.1 server — TLS complexity lives at the CDN edge.

```mermaid
flowchart LR
    Client["Browser / Client"]
    CF["CloudFront<br/>d3ahc2eiiqz0iu.cloudfront.net"]
    EC2["EC2<br/>54.83.192.65:80"]
    IPT["iptables<br/>PREROUTING 80 → 8080"]
    KS["kiss-server<br/>:8080"]

    Client -->|"HTTPS :443"| CF
    CF -->|"HTTP :80"| EC2
    EC2 --> IPT
    IPT --> KS
```

### Why CloudFront, Not Server-Side TLS

- **KISS philosophy:** The server's job is serving static files correctly. TLS is a separate concern
  best handled by infrastructure purpose-built for it.
- **Auto-renewing certificates:** ACM certificates renew automatically via DNS validation — no
  manual cert rotation, no downtime windows, no cron jobs.
- **CDN benefits:** CloudFront provides edge caching, gzip/brotli compression, and DDoS mitigation
  at no extra cost (free tier covers the project's traffic).
- **Separation of concerns:** The EC2 security group restricts port 80 to CloudFront IPs only.
  Direct HTTP access to the origin is blocked — all traffic must flow through HTTPS at the edge.
