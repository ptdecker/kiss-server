# ptodd

## What This Is

A from-scratch HTTP/1.1 static file server written in pure Rust with no external dependencies beyond the `log` crate facade. The server handles concurrent requests via a fixed thread pool, dispatches through a Handler/Context/Router abstraction, and serves static files with binary-safe reads, correct MIME detection, and path traversal prevention.

v1.0 shipped 2026-03-10: the server is fully functional for static file serving.

## Core Value

A client can request any static file (HTML, CSS, JS, WASM, etc.) by path and receive a correct, RFC-compliant HTTP/1.1 response — without crashing, leaking filesystem paths, or serving the wrong content type.

## Requirements

### Validated

- ✓ HTTP/1.1 request parsing (method + target + version) — existing pre-v1.0
- ✓ Thread pool for concurrent connection handling — existing pre-v1.0
- ✓ Typed HTTP method enum (GET, HEAD, POST, PUT, DELETE, etc.) — existing pre-v1.0
- ✓ Structured logging via `log` crate facade — existing pre-v1.0
- ✓ Custom DateTime with calendar arithmetic — existing pre-v1.0
- ✓ RFC-3986 percent-encoding utilities — existing pre-v1.0
- ✓ Response struct with status code, headers, and body — v1.0
- ✓ MIME type detection from file extension (10 types + octet-stream fallback) — v1.0
- ✓ Router abstraction (path → handler dispatch) — v1.0
- ✓ Static file serving from a configurable root directory (`--root` required) — v1.0
- ✓ Path traversal attack prevention (`canonicalize()` + `starts_with(root)`) — v1.0
- ✓ Proper HTTP/1.1 response headers (Content-Type, Content-Length, Date, Connection: close) — v1.0
- ✓ Request size limits (max 100 header lines) — v1.0
- ✓ Fix: crash on malformed/non-UTF-8 HTTP requests → 400 response — v1.0
- ✓ Fix: unsafe `unwrap_unchecked()` in DateTime → safe `?` propagation — v1.0
- ✓ Fix: panic in worker thread mutex poison handling → `unwrap_or_else` recovery — v1.0
- ✓ Fix: panic in ThreadPool Drop implementation → `let _ = thread.join()` — v1.0
- ✓ DateTime year/month calculation via arithmetic (Howard Hinnant `civil_from_days`) — v1.0
- ✓ Cargo.lock committed for reproducible builds — v1.0
- ✓ HEAD request support (headers only, no body) — v1.0
- ✓ Percent-decoded path routing — v1.0
- ✓ Dot-dot component rejection — v1.0

### Active

<!-- v1.1 Ops & Deployment — building toward these -->

- [ ] GitHub Actions CI pipeline (lint + build + test) required to pass before PR merge
- [ ] GitHub branch protection: PRs required for all changes to main
- [ ] AWS EC2 instance running kiss-server as a managed service
- [ ] ptodd.org domain routing (GoDaddy DNS + AWS networking) pointing to EC2
- [ ] "Hello World" static site deployed on EC2 instance
- [ ] GitHub CD pipeline: prod branch → deploy to EC2 + create GitHub release
- [ ] GitHub build status badge on repository
- [ ] CI/CD documentation in docs/ directory
- [ ] README.md updated to reflect project, CI/CD, and deployment

### Out of Scope

- REST endpoint handlers — router is designed for it; not built yet; candidate for v1.1
- Async I/O (tokio/async-std) — stays synchronous with thread pool
- TLS/HTTPS — out of scope for v1
- Authentication/authorization — intentionally open service
- Config file — CLI args are sufficient
- Metrics/observability beyond logging — future milestone
- Directory listing — serve files, not indexes
- Graceful SIGTERM shutdown — not achievable from safe `std` alone; SIGINT via Ctrl+C causes accept loop to error (accepted limitation)
- Middleware chain — designed for it (Handler trait), but not built

## Context

**v1.0 shipped 2026-03-10.** 2,525 lines of Rust (stdlib + `log` crate only). 6 phases, 16 plans, 106 commits.

Tech stack: Rust, stdlib only + `log` crate (v0.4.20).
Concurrency: Fixed thread pool, synchronous blocking I/O.

Known non-blocking tech debt from v1.0:
- Date header omitted on error paths if `DateTime::now()` fails (design tradeoff — no panic on error path; never fails in practice)
- `HEAD /` falls through to StaticFileHandler → 404 (correct HTTP behavior; register HEAD handler if needed)
- `pct_decode` dead code in `url/mod.rs` — remove or use in v2

## Constraints

- **Dependencies**: No new third-party crates — stdlib + `log` crate only
- **Protocol**: HTTP/1.1 only (no HTTP/2, no WebSockets)
- **Concurrency**: Synchronous blocking I/O with fixed thread pool (configurable size)

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Keep `log` crate | Already in use; minimal facade with zero runtime cost | ✓ Good — no issues |
| Router-first design | Static files are a route handler, not special-cased logic | ✓ Good — `set_fallback()` cleanly decouples static serving |
| Static root via CLI arg (`--root` required) | Most flexible; avoids hardcoding conventions; server refuses to start without it | ✓ Good — explicit is better than implicit |
| No async runtime | Learning project stays simple; thread pool sufficient for goals | ✓ Good — no tokio complexity needed |
| Howard Hinnant `civil_from_days` | O(1) Gregorian arithmetic, replaces iterative year/month loops | ✓ Good — clean, well-documented algorithm |
| `Result` propagation throughout | Eliminate all `unwrap()` on unhappy paths | ✓ Good — zero panics in production code paths |
| `Vec<(String,String)>` for headers | Preserves insertion order; no hashing overhead for <15 headers | ✓ Good — simple and correct |
| `write_to` consumes `Response` (`self`) | Prevents double-send; ownership enforces correct use at compile time | ✓ Good — Rust ownership solves accidental reuse |
| Handler trait `fn handle(&self, ctx: &mut Context)` | In-place mutation; no reconstruction overhead; enables future middleware | ✓ Good — extensible without structural changes |
| `add_header` (mutating) vs `header` (value-chaining) | Builder for construction, `add_header` for post-dispatch injection (Date header) | ✓ Good — clean separation of construction vs augmentation |
| Date header injected by `handle_connection`, not handlers | Cross-cutting concern owned by server layer; handlers don't need to know about it | ✓ Good — single responsibility |
| PATH-03 in `StaticFileHandler`, not Router | Requires configured root path; deferred correctly | ✓ Good — right layer of abstraction |
| `decoded_path()` uses byte-buffer with `hex_char_to_byte()` | Avoids `pct_decode()` multi-byte-per-call complexity | ✓ Good — simpler for sequential path byte processing |
| 404 on path traversal (not 403) | Avoids leaking whether the path exists outside root | ✓ Good — consistent with OWASP guidance |

## Current Milestone: v1.1 Ops & Deployment

**Goal:** Ship the CI/CD pipeline, AWS deployment, and domain routing so kiss-server runs live at ptodd.org with automated build verification and continuous deployment.

**Target features:**
- GitHub Actions CI (lint + build + test, required before merge)
- GitHub branch protection (PRs required for main)
- AWS EC2 running kiss-server as a managed service
- ptodd.org DNS + AWS networking → EC2
- Hello World static site on EC2
- CD pipeline (prod branch → EC2 deploy + GitHub release)
- GitHub build status badge
- CI/CD documentation in docs/
- README.md update

---
*Last updated: 2026-03-10 after v1.1 milestone start*
