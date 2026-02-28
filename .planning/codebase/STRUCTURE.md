# Codebase Structure

**Analysis Date:** 2026-02-28

## Directory Layout

```
ptodd/
├── src/                    # Rust source code
│   ├── main.rs            # Application entry point
│   ├── logger/            # Logging implementation
│   │   └── mod.rs         # SimpleLogger implementing log facade
│   ├── server/            # HTTP server implementation
│   │   ├── mod.rs         # Server struct and connection handler
│   │   ├── error.rs       # Server-specific error types
│   │   ├── pool.rs        # ThreadPool implementation
│   │   ├── worker.rs      # Worker thread management
│   │   └── request.rs     # HTTP request parsing
│   ├── time/              # Time utilities
│   │   ├── mod.rs         # DateTime struct and calendar calculations
│   │   └── error.rs       # Time-specific error types
│   └── url/               # URL parsing (minimal implementation)
│       └── mod.rs         # Url struct and percent-encoding utilities
├── Cargo.toml             # Package manifest and dependencies
├── Cargo.lock             # Locked dependency versions
├── Justfile               # Just automation recipes
├── README.md              # Project overview and setup instructions
├── HOSTING.md             # Deployment and hosting guidance
├── hello.html             # Default response for GET /
├── 404.html               # 404 error response page
└── target/                # Build artifacts (generated)
```

## Directory Purposes

**src/**
- Purpose: All Rust source code
- Contains: Module files (.rs) organized by concern
- Key files: `main.rs` (entry), `server/mod.rs` (core logic)

**src/logger/**
- Purpose: Custom logging implementation
- Contains: SimpleLogger struct implementing log crate facade
- Key files: `src/logger/mod.rs` - Provides structured logging with timestamps

**src/server/**
- Purpose: HTTP server implementation and request handling
- Contains: Server lifecycle, connection handling, HTTP parsing, thread pool
- Key files:
  - `src/server/mod.rs` - Server struct and handle_connection logic
  - `src/server/pool.rs` - ThreadPool for concurrent request handling
  - `src/server/request.rs` - HTTP request parsing and method enum
  - `src/server/error.rs` - Error type definitions

**src/time/**
- Purpose: Time and calendar utilities
- Contains: DateTime struct with calendar calculation helpers
- Key files: `src/time/mod.rs` - System time facade with leap year, day-of-year logic

**src/url/**
- Purpose: URL parsing and percent-encoding
- Contains: Url struct and RFC-3986 percent-encoding helpers
- Key files: `src/url/mod.rs` - Currently minimal URL wrapping, stub for future expansion

## Key File Locations

**Entry Points:**
- `src/main.rs`: Application entry point; initializes logger and starts server

**Configuration:**
- `Cargo.toml`: Package name, version (0.1.0), edition (2021), log crate dependency
- `Justfile`: Build automation commands

**Core Logic:**
- `src/server/mod.rs`: Server struct, connection handler, HTTP response logic
- `src/server/request.rs`: HTTP request parsing and method extraction
- `src/server/pool.rs`: Thread pool with worker thread management

**Static Content:**
- `hello.html`: Default response for GET / (200 OK)
- `404.html`: Response for unmatched routes (404 NOT FOUND)

**Testing:**
- `src/time/mod.rs` lines 254-270: Leap year calculation tests
- `src/url/mod.rs` lines 80-130: Percent-encoding encode/decode tests

## Naming Conventions

**Files:**
- Snake case: `main.rs`, `server.rs`, `pool.rs` (single concern modules)
- `mod.rs`: Private module implementations; public interface at module level
- `error.rs`: Error type definitions in dedicated file per module

**Directories:**
- Snake case: `logger/`, `server/`, `time/`, `url/`
- Semantic grouping: By concern/functionality

**Types:**
- PascalCase: `Server`, `ThreadPool`, `Worker`, `Request`, `RequestMethod`, `DateTime`, `SimpleLogger`, `Error`
- Enum variants: PascalCase: `Get`, `Post`, `January`, `February`

**Functions:**
- Snake case: `new()`, `run()`, `execute()`, `build()`, `parse()`, `init()`, `now()`
- Module functions: Private unless pub: `handle_connection()` (private to server mod)

**Constants:**
- UPPER_SNAKE: `DEFAULT_POOL_SIZE`, `DEFAULT_ADDR`, `LOG_ENV_VAR_NAME`

## Where to Add New Code

**New HTTP Route Handler:**
- Location: `src/server/mod.rs` in `handle_connection()` function (lines 88-95)
- Pattern: Add new match arm to route string; read file and return appropriate status/filename
- Error handling: Propagate errors up via `?` operator; logging already in place

**New Request Header Parser:**
- Location: Create new field in `Request` struct (`src/server/request.rs`)
- Pattern: Parse from `raw_request` Vec in `Request::parse()`; add to struct
- Error handling: Return InvalidRequest error variant for malformed data

**New Module/Utility:**
- Primary code: `src/{module_name}/mod.rs` (public interface) + `src/{module_name}/error.rs` (private errors)
- Registration: Add `mod {module_name};` and `pub use {module_name};` in `src/main.rs` or parent module
- Dependencies: Specify in parent module via `use` statements

**New Logging Point:**
- Pattern: Use `log::{debug!, info!, warn!}` macros (already configured via SimpleLogger)
- Location: Strategic points in `handle_connection()`, `ThreadPool::execute()`, worker loop
- Configuration: Set RUST_LOG env var; defaults to "trace" (all levels)

**Tests:**
- Location: Inline in same file as code (e.g., `src/url/mod.rs` lines 80-130)
- Pattern: Use `#[test]` attribute and standard assertion macros
- Run: `cargo test` (configured in Cargo.toml)

## Special Directories

**target/**
- Purpose: Build artifacts and compiled binaries
- Generated: Yes (created by `cargo build`)
- Committed: No (in .gitignore)

**.planning/codebase/**
- Purpose: Architecture and structure documentation
- Generated: No (manually maintained)
- Committed: Yes

**.github/**
- Purpose: GitHub-specific configuration (workflows, issues templates)
- Contents: Present but minimal
- Committed: Yes

**.idea/**
- Purpose: JetBrains IDE configuration (IntelliJ, CLion)
- Generated: IDE-specific
- Committed: Yes (some .idea files tracked)

**.vscode/**
- Purpose: VS Code editor configuration
- Generated: Editor-specific
- Committed: Yes (may contain workspace settings)

