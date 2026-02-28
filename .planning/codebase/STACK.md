# Technology Stack

**Analysis Date:** 2026-02-28

## Languages

**Primary:**
- Rust 2021 Edition - Backend HTTP server implementation

## Runtime

**Environment:**
- Rust toolchain with Cargo

**Package Manager:**
- Cargo (Rust package manager)
- Lockfile: `Cargo.lock` (present)

## Frameworks

**Core:**
- Custom HTTP/1.1 server implementation - Written from scratch in `src/server/mod.rs` with no third-party HTTP framework dependencies

**Testing:**
- Built-in Rust test framework - Unit tests embedded in source files using `#[cfg(test)]` modules

**Build/Dev:**
- Cargo - Standard Rust build tool

## Key Dependencies

**Critical:**
- `log` v0.4.20 - Logging facade for structured logging throughout the application
  - Features: `["std"]` enabled

## External Libraries

**Minimal dependencies approach:**
- The codebase explicitly minimizes third-party crate dependencies as stated in `src/server/mod.rs`: "It has no third-party crate dependencies" for the HTTP server
- Custom implementations for logger, URL parsing, and time utilities rather than relying on external crates

## Configuration

**Environment:**
- Logging configured via `RUST_LOG` environment variable in `src/logger/mod.rs`
- Default log level: "trace" if `RUST_LOG` not set
- Log output: stderr

**Build:**
- Standard Cargo.toml manifest at root: `/Users/todddecker/rust/ptodd/Cargo.toml`
- No custom build scripts configured

## Server Configuration

**Default Server Settings:**
- Binding address: `localhost:6502` (defined in `src/main.rs` as `DEFAULT_ADDR`)
- Thread pool size: 4 workers (defined in `src/server/mod.rs` as `DEFAULT_POOL_SIZE`)

## Platform Requirements

**Development:**
- Rust 1.56+ (supports 2021 edition)
- Standard development tools (git, cargo)

**Production:**
- Linux, macOS, or Windows system with standard libraries
- Compiled binary deployment model

## HTTP Protocol Support

**Implemented:**
- HTTP/1.1 basic implementation
- Request methods: GET, HEAD, POST, PUT, DELETE, CONNECT, OPTIONS, TRACE (RFC-9110 7.1 compliant)
- Request parsing and response generation

**Planned/TODO:**
- Full HTTP/1.1 specification compliance (RFC-9110, RFC-9111, RFC-9112)
- URI parsing (RFC-3986)
- Origin rules (RFC-6454)
- Response date headers (RFC-9110 6.6.1)

## Custom Implementations

**Logger:** `src/logger/mod.rs` - Custom implementation of the log crate's Log trait

**URL Parser:** `src/url/mod.rs` - Basic URL parser with RFC-3986 percent-encoding support

**Time Utilities:** `src/time/mod.rs` - DateTime handling with leap year calculations and date arithmetic

**Thread Pool:** `src/server/pool.rs` - Thread pool with configurable worker count for request handling

---

*Stack analysis: 2026-02-28*
