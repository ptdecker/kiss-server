# Architecture

**Analysis Date:** 2026-02-28

## Pattern Overview

**Overall:** Layered HTTP server with modular concerns separation.

**Key Characteristics:**
- Single-threaded entry point that delegates to thread pool for concurrent request handling
- Error handling follows Jeremy Chone's pattern with type-specific error variants
- Minimal external dependencies (only `log` crate as facade)
- Standard library-only implementation for core functionality
- Module-based organization with clear separation of concerns

## Layers

**Main/Orchestration:**
- Purpose: Application entry point and initialization
- Location: `src/main.rs`
- Contains: Program initialization, module composition
- Depends on: `logger::SimpleLogger`, `server::Server`
- Used by: Runtime/OS

**Server Layer:**
- Purpose: Manages HTTP server lifecycle, connection handling, request processing
- Location: `src/server/mod.rs`
- Contains: TCP listener, connection handler, HTTP request/response logic
- Depends on: `std::net`, `std::io`, `server::request`, `server::pool`, `url::Url`, `time::DateTime`
- Used by: Main

**Thread Pool Layer:**
- Purpose: Manages concurrent request handling through worker threads
- Location: `src/server/pool.rs` and `src/server/worker.rs`
- Contains: ThreadPool, Worker, Job abstraction
- Depends on: `std::sync::mpsc`, `std::sync::Mutex`, `std::thread`
- Used by: Server

**Request Parsing Layer:**
- Purpose: Parses HTTP request data into structured types
- Location: `src/server/request.rs`
- Contains: RequestMethod enum, Request struct with parse logic
- Depends on: `url::Url`, `server::error`
- Used by: Server connection handler

**Logging Layer:**
- Purpose: Provides structured logging facade implementing the `log` crate interface
- Location: `src/logger/mod.rs`
- Contains: SimpleLogger struct implementing Log trait
- Depends on: `log` crate, `time::DateTime`
- Used by: All modules via `log::*` macros

**Utility Layers:**
- **Time Module** (`src/time/mod.rs`): System time abstraction with calendar calculation (leap years, day-of-year, month/day conversion)
- **URL Module** (`src/url/mod.rs`): URL representation with RFC-3986 percent-encoding utilities (currently minimal implementation)

## Data Flow

**Connection Handling Flow:**

1. `Server::run()` binds TCP listener and iterates over incoming connections
2. `handle_connection()` reads HTTP request lines from TcpStream
3. Request bytes parsed into `Vec<String>` (HTTP headers)
4. `Request::parse()` extracts method and target from first line
5. `RequestMethod::try_from()` converts method string to enum
6. `Url::from()` wraps target path
7. Route matching determines response (200 OK for `/`, 404 for others)
8. File read and HTTP response constructed with status, headers, body
9. Response written to TcpStream and connection closed

**Concurrency Flow:**

1. `Server::run()` calls `ThreadPool::execute()` for each incoming connection
2. Closure passed to `execute()` wrapped in `Box<dyn FnOnce()>`
3. Job pushed to mpsc channel shared by all workers
4. Worker threads receive job from channel and execute closure
5. Workers continue looping until channel drops (server shutdown)

**Error Propagation:**

1. Result types flow up stack: `main()` returns `Result<()>`
2. Error variants: `server::error::Error` (InvalidRequest, Channel, Io)
3. Custom From implementations convert `std::io::Error` and `mpsc::SendError`
4. Warnings logged for recoverable errors; application continues
5. Fatal errors bubble to main and terminate with error display

## Key Abstractions

**RequestMethod Enum:**
- Purpose: Type-safe HTTP method representation
- Examples: `src/server/request.rs` lines 10-20
- Pattern: C-style enum with Display and TryFrom<&str> implementations

**Request Struct:**
- Purpose: Parsed HTTP request representation
- Examples: `src/server/request.rs` lines 55-63
- Pattern: Contains method (RequestMethod) and target (Url), immutable after parse

**ThreadPool:**
- Purpose: Bounded concurrency model with worker threads
- Examples: `src/server/pool.rs`
- Pattern: Generic job abstraction via `Box<dyn FnOnce() + Send + 'static>` allows executing any closure

**DateTime:**
- Purpose: System time representation with calendar calculations
- Examples: `src/time/mod.rs` lines 86-94
- Pattern: Facade around SystemTime with computed fields (year, month, day, day_of_year)

**SimpleLogger:**
- Purpose: Structured logging with timestamp, level, target, message
- Examples: `src/logger/mod.rs`
- Pattern: Implements `log::Log` trait for facade compatibility

## Entry Points

**main():**
- Location: `src/main.rs`
- Triggers: Program startup
- Responsibilities: Initialize logger, create server on default address (localhost:6502), start request loop

**handle_connection():**
- Location: `src/server/mod.rs` lines 76-106
- Triggers: Thread pool worker receives job for incoming connection
- Responsibilities: Read HTTP headers, parse request, route to handler, construct and send response

**SimpleLogger::init():**
- Location: `src/logger/mod.rs`
- Triggers: Called early in main()
- Responsibilities: Read RUST_LOG env var (default "trace"), register logger as global facade, set max level

## Error Handling

**Strategy:** Type-based error variants with From trait for conversion, warnings logged for connection errors, fatal errors propagate to main.

**Patterns:**
- Server errors: `server::error::Error` with variants InvalidRequest, Channel, Io
- Time errors: `time::error::Error` with variant InvalidMonth
- Global type alias: `type Result<T> = std::result::Result<T, Error>` provides short-hand
- Conversion: `From<std::io::Error>` and `From<mpsc::SendError<T>>` implemented for server::Error
- Recovery: `handle_connection().unwrap_or_else()` logs warning and continues on connection error
- Display: Error types implement fmt::Display for readable error messages

## Cross-Cutting Concerns

**Logging:** Uses `log` crate macros (info!, debug!, warn!) throughout. SimpleLogger formats with timestamp, level, target module, and message. Output to stderr.

**Validation:** HTTP request parsing validates: exact 3 parts in control line, HTTP version is 1.1, method is recognized enum variant.

**HTTP Compliance:** Follows RFC-9110 for HTTP semantics (method enum based on RFC 9110 7.1). TODO comments indicate planned support for HTTP/1.1 caching and URI RFC-3986.

