# External Integrations

**Analysis Date:** 2026-02-28

## APIs & External Services

**Not applicable** - This project has no external API integrations.

## Data Storage

**Databases:**
- Not used - Project is a basic HTTP server with no persistent storage layer

**File Storage:**
- Local filesystem only - Serves static HTML files (`hello.html`, `404.html`) from filesystem
- Location: Server root directory (files referenced in `src/server/mod.rs` handle_connection function)

**Caching:**
- Not implemented - No caching system in place

## Authentication & Identity

**Auth Provider:**
- Not implemented - No authentication system

## Monitoring & Observability

**Error Tracking:**
- Not used - Error handling is internal (see `src/server/error.rs`)

**Logs:**
- Stderr output via custom logger implementation
- Environment variable: `RUST_LOG` for level configuration
- Format: `{timestamp}: {level}: {target}: {message}`
- Implementation: `src/logger/mod.rs`

## CI/CD & Deployment

**Hosting:**
- Self-hosted binary deployment model
- Binds to localhost:6502 by default

**CI Pipeline:**
- GitHub Actions workflow: `.github/workflows/rust.yml`
- Triggers: Push to main branch, pull requests to main branch
- Steps:
  - Build: `cargo build --verbose`
  - Test: `cargo test --verbose`
- Environment: ubuntu-latest

## Environment Configuration

**Required env vars:**
- `RUST_LOG` (optional) - Set logging level (info, debug, trace, warn, error)
  - Default if not set: "trace"

**Secrets location:**
- Not applicable - No secrets management required

## Webhooks & Callbacks

**Incoming:**
- Not implemented - HTTP server does not handle webhooks

**Outgoing:**
- Not implemented - No outbound webhook notifications

## Network Configuration

**TCP Server:**
- Listens on `localhost:6502` by default (configurable via DEFAULT_ADDR in `src/main.rs`)
- Uses standard Rust std::net::TcpListener for binding
- TcpStream for connection handling

## HTTP Endpoint Behavior

**Static Routes (hardcoded in `src/server/mod.rs`):**
- `GET /` - Returns `hello.html` with HTTP 200
- `GET /sleep` - Sleeps 5 seconds, then returns `hello.html` with HTTP 200 (test endpoint for thread pool behavior)
- All other routes - Return `404.html` with HTTP 404 NOT FOUND

**No dynamic routing or configuration available.**

---

*Integration audit: 2026-02-28*
