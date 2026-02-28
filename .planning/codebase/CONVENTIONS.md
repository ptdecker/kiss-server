# Coding Conventions

**Analysis Date:** 2026-02-28

## Naming Patterns

**Files:**
- Module files: `mod.rs` pattern for module root exports
- Submodules: `{module_name}.rs` (e.g., `error.rs`, `pool.rs`, `request.rs`, `worker.rs`)
- Convention: all lowercase with underscores, matching Rust conventions

**Functions:**
- Lowercase with underscores: `fn new()`, `fn parse()`, `fn execute()`, `fn is_leap_year()`, `fn days_in_month()`
- Private helper functions: lowercase with underscores, e.g., `fn hex_char_to_byte()`, `fn pct_encode()`, `fn pct_decode()`, `fn year()`, `fn month()`
- Public API methods: descriptive verb-based names

**Variables:**
- Lowercase with underscores: `epoch_seconds`, `remaining_days`, `status_line`, `log_level`, `day_of_year`
- Constants: UPPERCASE with underscores: `DEFAULT_ADDR`, `DEFAULT_POOL_SIZE`, `LOG_ENV_VAR_NAME`
- Type parameters: single uppercase letters: `T`, `U`, `F`

**Types:**
- PascalCase for structs: `Server`, `Request`, `ThreadPool`, `Worker`, `DateTime`, `SimpleLogger`, `RequestMethod`
- PascalCase for enums: `Month`, `Error`, `RequestMethod`, `LevelFilter`
- PascalCase for traits: `Log` (from standard library)

## Code Style

**Formatting:**
- Rust standard formatting (as enforced by `rustfmt`)
- 4-space indentation (Rust default)
- Line width follows standard conventions (typically 100-120 characters)
- No custom `.rustfmt.toml` detected; uses Rust defaults

**Linting:**
- No explicit clippy config file detected; relies on standard clippy rules
- Minimal linting overrides: `#![allow(unused)]` in `src/url/mod.rs` and `src/time/mod.rs` for intentionally unused code awaiting future implementation

## Import Organization

**Order:**
1. Standard library imports (`std::`)
2. Third-party crate imports (`log::`)
3. Local module imports (relative paths with `use super::*;` and explicit submodule imports)

**Examples from codebase:**
```rust
// src/main.rs
use log::{debug, info, warn};
use logger::SimpleLogger;
use server::Server;

mod logger;
mod server;
mod time;
mod url;
```

```rust
// src/server/request.rs
use crate::url::Url;
use super::*;
```

**Path Aliases:**
- No custom path aliases configured; uses standard module structure
- Relative imports with `use super::*;` common for accessing parent module items
- Full crate paths with `crate::` for cross-module access (e.g., `crate::url::Url`, `crate::time::DateTime`)

## Error Handling

**Patterns:**
- Custom `Error` enums with `Display` and `std::error::Error` trait implementations
- Module-specific error types: `server::error::Error`, `time::error::Error`
- Result type aliases: `pub type Result<T> = std::result::Result<T, Error>;` in each module
- From trait implementations for error conversion: `From<std::io::Error>`, `From<mpsc::SendError<T>>`
- Error handling with `.unwrap_or_else()` for recoverable errors in production paths: `unwrap_or_else(|e| warn!("...", e))`
- Some unsafe unwraps in private functions with justification (e.g., `unsafe { now.duration_since(UNIX_EPOCH).unwrap_unchecked() }`)
- Match statements for explicit error handling, rarely using `?` operator

**Example pattern:**
```rust
// src/server/error.rs
pub type Result<T> = result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    InvalidRequest(String),
    Channel(String),
    Io(std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::InvalidRequest(e) => write!(f, "invalid request: {e}"),
            Error::Channel(s) => write!(f, "channel: {s}"),
            Error::Io(e) => write!(f, "io: {e}"),
        }
    }
}

impl std::error::Error for Error {}
```

## Logging

**Framework:** `log` crate facade (version 0.4.20)

**Custom Implementation:** `SimpleLogger` in `src/logger/mod.rs` provides minimal logging implementation that outputs to stderr

**Patterns:**
- Use `log::info!()` for informational messages: `info!("Listening for connections on {}", &self.addr)`
- Use `log::debug!()` for detailed debugging: `debug!("Request ({:?}): {:#?}", stream.peer_addr()?, http_request)`
- Use `log::warn!()` for recoverable errors: `warn!("handle_connection: {}", e)`
- Logging level controlled via `RUST_LOG` environment variable (defaults to "trace")
- All imports use specific log macros rather than glob imports: `use log::{debug, info, warn};`

## Comments

**When to Comment:**
- Module-level documentation with `//!` prefix explaining module purpose: "Simple logger implementation", "HTTP Request (v1.1)"
- Function-level documentation with `///` prefix for public APIs
- RFC references and specification links in comments for complex functionality
- TODO and FIXME comments for incomplete features:
  - `// TODO: HTTP/1.1 Support` with RFC references
  - `// TODO: remove unused linting override` when code stubs are intentional
- Inline comments explaining complex encoding/decoding logic

**Documentation Examples:**
```rust
//! Provides the backend implementation for the ptodd.org website.

//! A basic URL parser with normalization

/// Request Methods (RFC-9110 7.1)
/// Cf. <https://datatracker.ietf.org/doc/html/rfc9110#name-overview>
#[derive(Debug, Copy, Clone)]
pub(super) enum RequestMethod {
    Get,
    Head,
    Post,
    // ...
}
```

**JSDoc/TSDoc:**
- Not applicable; Rust uses `///` and `//!` doc comments
- Doc comments follow Markdown convention
- HTML links and code examples supported in documentation

## Function Design

**Size:**
- Generally compact functions (5-40 lines)
- Single-responsibility principle observed
- Helper functions broken out for reusability: `hex_char_to_byte()`, `pct_encode()`, `pct_decode()`

**Parameters:**
- Use of trait bounds for flexible types: `impl Into<String>`, `impl Into<T>` where `T: Into<u16>`
- Generic parameters with bounds: `T: Into<u16>`, `F: FnOnce() + Send + 'static`
- Ownership-aware parameter design (uses refs and owned values appropriately)

**Return Values:**
- Result types using custom `Result<T>` aliases for error propagation
- Option types where appropriate (e.g., `pub fn next_month(&self) -> Month`)
- Unit return `()` for operations with side effects
- Explicit `Ok(())` for success states in fallible operations

**Example:**
```rust
impl RequestMethod {
    pub fn try_from(value: &str) -> Result<Self> {
        match value {
            "GET" => Ok(RequestMethod::Get),
            // ...
            _ => Err(Error::InvalidRequest(format!("invalid method: {value}"))),
        }
    }
}
```

## Module Design

**Exports:**
- Explicit `pub use` statements in module files: `pub use error::{Error, Result};`
- `pub(super)` for intra-module visibility (e.g., `pub(super) struct Request`, `pub(super) enum RequestMethod`)
- Public API clearly separated from internal implementation
- Type re-exports at module root for ergonomic access

**Barrel Files:**
- `mod.rs` files act as module entry points
- Typical pattern: module definition + public type re-exports + documentation
- Sub-modules defined but not re-exported if they're internal implementation

**Example structure:**
```rust
// src/server/mod.rs
pub use error::{Error, Result};
use pool::ThreadPool;
use request::Request;

mod error;
mod pool;
mod request;
mod worker;
```

---

*Convention analysis: 2026-02-28*
