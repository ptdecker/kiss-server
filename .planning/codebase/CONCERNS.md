# Codebase Concerns

**Analysis Date:** 2026-02-28

## Tech Debt

**Incomplete HTTP/1.1 Protocol Implementation:**
- Issue: Only basic HTTP/1.1 support; missing critical features like persistent connections, pipelining, chunked transfer encoding, compression, and proper header handling
- Files: `src/server/mod.rs` (lines 30-36, commented TODO)
- Impact: Cannot handle modern HTTP clients properly; will fail with any client expecting standard HTTP/1.1 features; responses lack required headers (Content-Type, Date, Server, etc.)
- Fix approach: Implement RFC-9110 (HTTP Semantics), RFC-9112 (HTTP/1.1), and RFC-9111 (Caching) specifications; add header parsing and generation; implement connection management

**URI/URL Parsing Not Implemented:**
- Issue: URL module is stubbed out with only basic path storage; no actual URI parsing per RFC-3986
- Files: `src/url/mod.rs` (lines 3-4 - `#![allow(unused)]` indicates unused code)
- Impact: Cannot properly validate or normalize URLs; percent-encoding functions defined but unused; no support for query strings, fragments, or absolute URIs
- Fix approach: Implement RFC-3986 URI parsing; activate unused encoding/decoding functions; add proper URL validation and normalization

**Unused/Dead Code:**
- Issue: Multiple modules have `#![allow(unused)]` linting overrides with unimplemented functionality
- Files: `src/url/mod.rs` (line 3), `src/time/mod.rs` (line 6)
- Impact: Code exists but serves no purpose; increases maintenance burden; creates confusion about what's actually functional
- Fix approach: Either implement the missing functionality or remove entirely; remove blanket `allow(unused)` directives

**Unsafe Code in DateTime:**
- Issue: `unsafe { unwrap_unchecked() }` used to bypass error checking
- Files: `src/time/mod.rs` (line 216)
- Impact: If `duration_since(UNIX_EPOCH)` returns an error (which could happen with certain system clock configurations), the program will panic with undefined behavior instead of graceful error handling
- Fix approach: Replace `unsafe { unwrap_unchecked() }` with proper error propagation using `?` operator

## Known Bugs

**Hardcoded File Paths:**
- Symptoms: Server crashes if `hello.html` or `404.html` are not in the exact current working directory
- Files: `src/server/mod.rs` (lines 89, 94, 96)
- Trigger: Run server from any directory except the project root
- Workaround: Always run from project root; none in code
- Impact: High - affects deployability and usability

**Request Parsing Panics on Malformed Input:**
- Symptoms: `.unwrap()` on line 80 will panic if any line can't be read as UTF-8
- Files: `src/server/mod.rs` (line 80)
- Trigger: Send binary data or invalid UTF-8 in HTTP request
- Workaround: Ensure only valid UTF-8 requests are sent
- Impact: Medium - DoS vector; any invalid request crashes the server

**Stream Peer Address Access Without Error Handling:**
- Symptoms: `stream.peer_addr()?` fails but error message only warns instead of properly handling
- Files: `src/server/mod.rs` (line 87)
- Trigger: Connection tracking on systems where peer address is unavailable
- Impact: Low - mostly informational logging, graceful degradation

## Security Considerations

**Synchronous Request Blocking:**
- Risk: `/sleep` endpoint blocks entire thread for 5 seconds; with 4-thread pool, 5 concurrent sleep requests DoS the server
- Files: `src/server/mod.rs` (lines 90-93)
- Current mitigation: None; intentional test code
- Recommendations: Remove test endpoints before production; use timeout handling; consider async implementation

**No Input Validation on HTTP Requests:**
- Risk: Path traversal attacks possible; crafted requests like `GET /../../../etc/passwd HTTP/1.1` could be processed
- Files: `src/server/mod.rs` (lines 88-95), `src/url/mod.rs` (incomplete parsing)
- Current mitigation: Only serving static files, but URL parsing is incomplete
- Recommendations: Implement proper URI parsing per RFC-3986; validate and normalize all paths; implement allowlist of acceptable routes

**No Authentication or Authorization:**
- Risk: All endpoints accessible to anyone
- Files: All server endpoints in `src/server/mod.rs`
- Current mitigation: None - service is public by design
- Recommendations: Add authentication framework if needed in future; document that this is an intentionally open service

**Resource Exhaustion - No Request Size Limits:**
- Risk: No limit on request header size; attacker can send infinitely large headers
- Files: `src/server/mod.rs` (lines 78-82)
- Current mitigation: None
- Recommendations: Add maximum header size enforcement; implement request timeout

**Stream Write Errors Silently Ignored:**
- Risk: Network errors during response writing are logged but don't propagate; client may receive incomplete responses
- Files: `src/server/mod.rs` (line 98)
- Current mitigation: Error is returned but caller only logs with `unwrap_or_else`
- Recommendations: Implement proper response validation; ensure complete writes before returning success

## Performance Bottlenecks

**Year Calculation via Brute Force Iteration:**
- Problem: Linear iteration from 1970 to current year to calculate date from epoch days
- Files: `src/time/mod.rs` (lines 169-190)
- Cause: Naive loop instead of mathematical formula
- Impact: ~50+ iterations per DateTime::now() call; CPU waste increases with each year
- Improvement path: Replace loop with calendar arithmetic or lookup table

**Month Calculation via Sequential Iteration:**
- Problem: Loop through all months to find current month
- Files: `src/time/mod.rs` (lines 195-210)
- Cause: Naive iteration instead of direct calculation
- Impact: Up to 12 iterations per DateTime::now() call; unnecessary when month can be calculated directly
- Improvement path: Convert to mathematical calculation using day_of_year

**Thread Pool Uses Arc<Mutex<>> for Receiver:**
- Problem: All worker threads contend on single Mutex for job queue
- Files: `src/server/pool.rs` (line 31), `src/server/worker.rs` (lines 17-20)
- Cause: Simple implementation choice; acceptable for 4-thread pool but doesn't scale
- Impact: Lock contention increases with worker count; each job acquisition requires mutex lock
- Improvement path: Use lockfree queue (crossbeam-queue) or work-stealing design for larger pools

**Redundant String Conversions in Logging:**
- Problem: DateTime struct formatted to string on every log line
- Files: `src/logger/mod.rs` (lines 58)
- Cause: Calls DateTime::now() for each log entry regardless of whether it will be output
- Impact: Date calculation performed even for filtered-out log levels
- Improvement path: Lazy-format timestamps only when enabled; cache recent timestamps

## Fragile Areas

**HTTP Response Formatting is Hardcoded:**
- Files: `src/server/mod.rs` (lines 98-104)
- Why fragile: Inline format string with no abstraction; adding headers requires modifying core function; no validation that response is RFC-compliant
- Safe modification: Create Response struct; add builder pattern; add header validation
- Test coverage: No tests for response formatting; edge cases like non-ASCII filenames untested

**URL Module Implementation Mismatch:**
- Files: `src/url/mod.rs`
- Why fragile: Encoding/decoding functions exist but are never called; module accepts any string without parsing; type suggests parsing happens but doesn't
- Safe modification: Either complete RFC-3986 implementation or simplify module to pure utilities; remove unused code path
- Test coverage: Tests exist for encoding/decoding but not for the actual Url struct usage

**Worker Thread Error Handling:**
- Files: `src/server/worker.rs` (line 19)
- Why fragile: `.expect("unable to lock spawned thread")` will panic if mutex is poisoned; one worker panic doesn't isolate failure
- Safe modification: Use `match` on lock result; handle poison errors gracefully; add panic recovery
- Test coverage: No tests for poisoned mutex scenario

**Thread Pool Drop Implementation:**
- Files: `src/server/pool.rs` (line 63)
- Why fragile: `.expect()` in Drop implementation will panic during unwinding if thread.join() fails; can cause double-panic
- Safe modification: Never panic in Drop; use `let _ = thread.join()` to ignore join errors gracefully
- Test coverage: No tests for abnormal shutdown scenarios

## Scaling Limits

**Fixed 4-Thread Pool:**
- Current capacity: 4 concurrent connections
- Limit: 5th concurrent request must wait for a thread to free
- Scaling path: Make pool size configurable via environment variable or config file; benchmark optimal size for target hardware; consider async runtime for higher concurrency

**Blocking I/O Implementation:**
- Current capacity: Cannot handle more than 4 concurrent connections efficiently
- Limit: Cannot scale beyond the thread pool size without spawning more threads (expensive)
- Scaling path: Migrate to async runtime (tokio/async-std); implement non-blocking I/O

**Memory Per Worker:**
- Current capacity: Each worker thread consumes ~2MB stack space
- Limit: 1000+ threads becomes problematic on typical systems
- Scaling path: Design assumes small thread pool; if async migration happens, this becomes non-issue

## Dependencies at Risk

**Log Crate - Facade with Single Implementation:**
- Risk: Using log facade but only implementing SimpleLogger; cannot swap logging at runtime
- Impact: Hard to implement metrics, structured logging, or log filtering without code changes
- Migration plan: Already using abstraction correctly; can add new Log implementations without code changes

**No Dependency Lock File Committed:**
- Risk: `Cargo.lock` exists but not in git (untracked), so reproducible builds may differ
- Impact: Different machines could compile slightly different binaries due to patch version variance
- Mitigation: `Cargo.lock` should be committed for binary projects (not libraries)

## Missing Critical Features

**No Configuration System:**
- Problem: All configuration hardcoded (address, pool size, log level via env var only)
- Blocks: Cannot easily deploy to different environments; no config file support; CLI args not supported
- Impact: Every deployment requires code changes or shell environment manipulation

**No Graceful Shutdown:**
- Problem: Server runs forever; no signal handling (SIGTERM/SIGINT)
- Blocks: Cannot cleanly stop server; will hang on connections; testing startup/shutdown is difficult
- Impact: Requires `kill -9`; risk of corrupted state if stopped abruptly

**No Metrics or Observability:**
- Problem: Only basic logging; no request counting, response time tracking, or error rates
- Blocks: Cannot monitor performance or diagnose issues in production
- Impact: Black box operation; difficult to detect problems until users report them

**No Testing Infrastructure:**
- Problem: Code exists but no test harness or integration tests
- Blocks: Cannot verify HTTP correctness; no regression testing; brittle to refactoring
- Impact: Changes risk breaking functionality; no confidence in correctness

**No Documentation:**
- Problem: Comments explain what code does but not why or how to use it
- Blocks: Difficult for new contributors to understand architecture; no deployment guide
- Impact: Knowledge locked in code; difficult to maintain long-term

## Test Coverage Gaps

**HTTP Request Parsing Not Tested:**
- What's not tested: Malformed requests, edge cases, RFC compliance
- Files: `src/server/request.rs` - no tests exist; only line 66 parses requests
- Risk: Crashes on invalid requests go unnoticed; silent bugs in parsing
- Priority: High - core functionality

**Response Formatting Not Tested:**
- What's not tested: Character encoding, large files, header generation, protocol compliance
- Files: `src/server/mod.rs` (lines 88-105) - no tests
- Risk: Sending broken responses without detection
- Priority: High - directly affects clients

**URL Parsing Logic Not Tested in Context:**
- What's not tested: How Url type is used throughout; percent-encoding integration
- Files: `src/url/mod.rs` - unit tests exist for helpers but not Url struct behavior
- Risk: Integration bugs between URL parsing and server routing
- Priority: Medium - once URL parsing is implemented

**Thread Pool Failure Scenarios Not Tested:**
- What's not tested: Worker thread panic, mutex poisoning, channel failure, shutdown under load
- Files: `src/server/pool.rs`, `src/server/worker.rs` - no tests
- Risk: Unexpected shutdowns or hangs in production
- Priority: High - affects reliability

**DateTime Calculation Accuracy Not Tested:**
- What's not tested: Leap years throughout history, epoch boundary conditions, year transitions
- Files: `src/time/mod.rs` - leap year tests exist but not full DateTime calculation
- Risk: Dates in logs could be incorrect on certain dates
- Priority: Medium - only affects logging accuracy

**Logger Edge Cases Not Tested:**
- What's not tested: Invalid log levels, logging during initialization, concurrent logging
- Files: `src/logger/mod.rs` - no tests
- Risk: Logging failures silently ignored
- Priority: Low - only affects debugging capability

---

*Concerns audit: 2026-02-28*
