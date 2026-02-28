# ptodd

## What This Is

A from-scratch HTTP/1.1 static file server written in pure Rust with no external dependencies beyond the `log` crate facade. The server handles concurrent requests via a thread pool and is designed around a router abstraction that starts with static file serving and is extensible to REST endpoints.

## Core Value

A client can request any static file (HTML, CSS, JS, WASM, etc.) by path and receive a correct, RFC-compliant HTTP/1.1 response — without crashing, leaking filesystem paths, or serving the wrong content type.

## Requirements

### Validated

- ✓ HTTP/1.1 request parsing (method + target + version) — existing
- ✓ Thread pool for concurrent connection handling — existing
- ✓ Typed HTTP method enum (GET, HEAD, POST, PUT, DELETE, etc.) — existing
- ✓ Structured logging via `log` crate facade — existing
- ✓ Custom DateTime with calendar arithmetic — existing
- ✓ RFC-3986 percent-encoding utilities — existing

### Active

- [ ] Response struct with status code, headers, and body
- [ ] MIME type detection from file extension
- [ ] Router abstraction (path → handler dispatch)
- [ ] Static file serving from a configurable root directory
- [ ] RFC-3986 URI parsing and path normalization
- [ ] Path traversal attack prevention
- [ ] Proper HTTP/1.1 response headers (Content-Type, Content-Length, Date)
- [ ] Request size limits (max header size enforcement)
- [ ] Graceful shutdown on SIGTERM/SIGINT
- [ ] Fix: crash on malformed/non-UTF-8 HTTP requests
- [ ] Fix: unsafe `unwrap_unchecked()` in DateTime
- [ ] Fix: panic in worker thread mutex poison handling
- [ ] Fix: panic in ThreadPool Drop implementation
- [ ] Fix: hardcoded file paths in server handler
- [ ] DateTime year/month calculation via arithmetic (not iteration)
- [ ] Cargo.lock committed for reproducible builds

### Out of Scope

- REST endpoint handlers — router is designed for it but not built yet
- Async I/O (tokio/async-std) — stays synchronous with thread pool
- TLS/HTTPS — out of scope for v1
- Authentication/authorization — intentionally open service
- Config file — address and pool size via CLI args or env vars is sufficient
- Metrics/observability beyond logging — future milestone
- Directory listing — serve files, not indexes

## Context

This is a Rust learning project ("ptodd"). The existing codebase has the skeleton of an HTTP server: request parsing, thread pool, logger, and time utilities. The `log` crate (v0.4.20) is the only external dependency and is kept. All other functionality is implemented from scratch.

The router should be designed as an extension point — static file serving is the first handler registered, but the interface should allow REST handlers to be added later without structural changes.

## Constraints

- **Dependencies**: No new third-party crates — stdlib + `log` crate only
- **Protocol**: HTTP/1.1 only (no HTTP/2, no WebSockets)
- **Concurrency**: Synchronous blocking I/O with fixed thread pool (configurable size)

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Keep `log` crate | Already in use; minimal facade with zero runtime cost | — Pending |
| Router-first design | Static files are a route handler, not special-cased logic | — Pending |
| Static root via CLI arg | Most flexible; avoids hardcoding conventions | — Pending |
| No async runtime | Learning project stays simple; thread pool sufficient for goals | — Pending |

---
*Last updated: 2026-02-28 after initialization*
