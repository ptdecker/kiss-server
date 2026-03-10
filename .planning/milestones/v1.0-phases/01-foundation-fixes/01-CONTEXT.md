# Phase 1: Foundation Fixes - Context

**Gathered:** 2026-02-28
**Status:** Ready for planning

<domain>
## Phase Boundary

Fix existing broken/unsafe code: eliminate crash vectors, DoS exposure, and unsafe datetime code. No new capabilities added. Server must handle bad input, resist simple DoS, and have no unsafe code in the datetime utilities.

Requirements in scope: SAFE-02, SAFE-03, SAFE-04, SAFE-05, SAFE-06, TIME-01, TIME-02

</domain>

<decisions>
## Implementation Decisions

### Header Line Limit (SAFE-05)
- Enforce limit inside `Request::parse()` — not in `handle_connection()`
- Use a named constant: `MAX_HEADER_LINES: usize = 100`
- Add new error variant `RequestTooLarge` to `server::error::Error` (distinct from `InvalidRequest`)
- When limit exceeded: propagate as `Err(Error::RequestTooLarge)` — existing handler logs warning and closes connection; client gets EOF

### DateTime Error Propagation (SAFE-02)
- Change `DateTime::now()` return type from `DateTime` to `Result<DateTime>`
- Add new error variant `SystemTime(std::time::SystemTimeError)` to `time::error::Error`
- Update callers (SimpleLogger, handle_connection) to handle the Result
- SimpleLogger on error: emit log line with `[unknown]` as timestamp — don't silence the log
- Remove `#![allow(unused)]` from `src/time/mod.rs` if no unused code remains after fixes

### Worker Mutex Poison (SAFE-03)
- Worker threads handle poisoned mutex without panicking — recover with `into_inner()` or log-and-continue
- Claude's discretion on exact recovery mechanism, but workers must not cascade into killing the whole pool

### ThreadPool Drop (SAFE-04)
- Use `let _ = thread.join()` — never call `.expect()` in a `Drop` impl

### DateTime Arithmetic (TIME-01, TIME-02)
- Replace year loop (iterating from 1970) and month loop (iterating through months) with arithmetic formulas
- Claude's discretion on exact formula — Zeller's congruence, Julian day math, or equivalent

### Test Coverage
- Unit tests alongside every fix (not deferred to later phases)
- Tests live in `#[cfg(test)]` blocks in the same module as the code being fixed
- `Request::parse()` test: returns `Err` on bad/binary input (does not panic)
- Thread pool test: poisoned mutex handled without panicking; `Drop` doesn't double-panic
- DateTime arithmetic test: spot-check known epoch → date conversions (epoch 0, a leap year boundary, a recent date)

### Routing Simplification (removing /sleep)
- Remove the `/sleep` endpoint — it's a DoS vector (5 concurrent requests fill the 4-thread pool)
- Remove hardcoded file reads (`hello.html`, `404.html`) — eliminates CWD-dependent crash
- `/` route returns a plain `200 OK` with a minimal text body (placeholder until Phase 5)
- All unmatched routes: close connection without sending a response (routing is Phase 3's job)

### Claude's Discretion
- Exact arithmetic formula for year and month calculation
- Exact text of the placeholder 200 OK body for `/`
- Exact recovery mechanism for poisoned mutex (recover data vs skip job)
- Whether `src/url/mod.rs` `#![allow(unused)]` should also be cleaned up if touched

</decisions>

<specifics>
## Specific Ideas

- No specific external references — standard Rust patterns apply
- SAFE-01 (returning 400 Bad Request) is Phase 2; Phase 1 only needs to not panic — closing with EOF on bad input is acceptable
- The Cargo.lock commit (SAFE-06) is a trivial git operation, not a code change

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `server::error::Error` enum (`src/server/error.rs`) — extend with `RequestTooLarge` variant
- `time::error::Error` enum (`src/time/mod.rs`) — extend with `SystemTime(std::time::SystemTimeError)` variant
- Existing `Result<T>` type aliases in each module — callers of `DateTime::now()` are `SimpleLogger` and `handle_connection()`

### Established Patterns
- Error enums follow the pattern: named variants, `Display` impl, `std::error::Error` impl, `From<std::X>` conversions
- Constants are `SCREAMING_SNAKE_CASE` (e.g., `DEFAULT_ADDR`, `DEFAULT_POOL_SIZE`)
- Unit tests use `#[cfg(test)]` blocks in the same file as the code under test
- Error recovery uses `.unwrap_or_else(|e| warn!(...))` for recoverable paths

### Integration Points
- `DateTime::now()` called in: `src/logger/mod.rs` (SimpleLogger::log), `src/server/mod.rs` (handle_connection response)
- `Request::parse()` receives raw lines already collected in `handle_connection()` — limit enforcement moves into parse()
- `ThreadPool` is constructed in `Server::new()` — Drop behavior is in `src/server/pool.rs`

</code_context>

<deferred>
## Deferred Ideas

- None — discussion stayed within phase scope

</deferred>

---

*Phase: 01-foundation-fixes*
*Context gathered: 2026-02-28*
