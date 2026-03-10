---
phase: 01-foundation-fixes
plan: "02"
subsystem: server
tags: [rust, http, server, safety, thread-pool, utf8, dos-prevention]

# Dependency graph
requires: []
provides:
  - "RequestTooLarge error variant with Display arm in src/server/error.rs"
  - "MAX_HEADER_LINES constant (100) in src/server/request.rs with limit enforcement"
  - "Poison-safe mutex lock recovery in worker thread (unwrap_or_else + into_inner)"
  - "Panic-safe ThreadPool Drop (let _ = thread.join())"
  - "Bounded UTF-8-safe header collection loop using read_line in handle_connection"
  - "Minimal routing stub: GET / returns 200 OK with body OK; unmatched routes close silently"
affects: [02-response-layer, 03-routing, 05-static-file-handler]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "BufReader::read_line loop for bounded header collection (not .lines().map(|r| r.unwrap()))"
    - "MutexGuard poison recovery via unwrap_or_else(|e| e.into_inner())"
    - "let _ = thread.join() in Drop impls to discard join errors"
    - "Return Err(e.into()) on I/O error in connection handler instead of panic"

key-files:
  created: []
  modified:
    - src/server/error.rs
    - src/server/request.rs
    - src/server/worker.rs
    - src/server/pool.rs
    - src/server/mod.rs
    - src/time/mod.rs
    - src/time/error.rs
    - src/logger/mod.rs

key-decisions:
  - "Use read_line loop (not .lines() iterator) for bounded header collection; .lines() cannot bound and panics on invalid UTF-8"
  - "Unmatched routes close connection silently (no 404 response) — routing is Phase 3 responsibility"
  - "MAX_HEADER_LINES enforced in both collection loop and parse() as defense-in-depth"
  - "Pre-existing compile failures in time/logger modules fixed as Rule 3 deviation (blocked all test runs)"

patterns-established:
  - "Bounded I/O: use read_line with explicit loop counter instead of unbounded iterators"
  - "Poison recovery: unwrap_or_else(|e| e.into_inner()) on Mutex::lock()"
  - "Drop safety: let _ = thread.join() — never .expect() inside Drop"

requirements-completed: [SAFE-03, SAFE-04, SAFE-05, SAFE-06]

# Metrics
duration: 4min
completed: 2026-03-01
---

# Phase 1 Plan 02: Foundation Fixes - Server Safety Summary

**Five crash vectors eliminated: UTF-8 panic, mutex poison cascade, double-panic Drop, unbounded header DoS, and /sleep DoS route removed; all replaced with safe patterns and verified by 13 passing unit tests**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-01T23:42:52Z
- **Completed:** 2026-03-01T23:47:06Z
- **Tasks:** 2
- **Files modified:** 8

## Accomplishments
- Added `RequestTooLarge` error variant with Display arm; `MAX_HEADER_LINES = 100` constant enforced in both collection loop and `Request::parse()`
- Replaced `.expect()` on mutex lock with `unwrap_or_else(|e| e.into_inner())` poison recovery in worker thread
- Replaced `.expect()` on `thread.join()` in `ThreadPool::Drop` with `let _ = thread.join()` to prevent double-panic
- Replaced unbounded `.lines().map(|r| r.unwrap())` collection with bounded `read_line` loop returning `Err` on invalid UTF-8
- Removed `/sleep` DoS route, `fs::read_to_string`, `hello.html`/`404.html` references; `GET /` now returns plain `200 OK`
- Verified SAFE-06: `Cargo.lock` is tracked in git (`git ls-files Cargo.lock` outputs `Cargo.lock`)

## Task Commits

Each task was committed atomically:

1. **Task 1: RequestTooLarge error variant and header line limit** - `2cdef4a` (feat)
2. **Task 2: Fix worker mutex poison, ThreadPool Drop, handle_connection** - `f631262` (fix)

## Files Created/Modified
- `src/server/error.rs` - Added `RequestTooLarge` variant and Display arm
- `src/server/request.rs` - Added `MAX_HEADER_LINES` const, limit check in `parse()`, 3 unit tests
- `src/server/worker.rs` - Mutex poison recovery with `unwrap_or_else(|e| e.into_inner())`
- `src/server/pool.rs` - Panic-safe Drop: `let _ = thread.join()`
- `src/server/mod.rs` - Bounded read_line loop, minimal routing, 2 TCP-level tests; removed fs/sleep/Duration imports
- `src/time/mod.rs` - Fixed pre-existing broken tests and added `#[allow(dead_code)]` to future-use public APIs (deviation)
- `src/time/error.rs` - Added `SystemTime` error variant (pre-existing unstaged change needed for compilation)
- `src/logger/mod.rs` - Updated `DateTime::now()` call for new `Result<DateTime>` return type (pre-existing unstaged change)

## Decisions Made
- Used `read_line` loop instead of `.lines()` iterator for header collection — `.lines()` cannot be bounded and panics on invalid UTF-8
- Unmatched routes close connection silently (no 404) — routing is Phase 3's responsibility; adding a 404 response now would require a Response type that doesn't exist yet
- `MAX_HEADER_LINES` enforced in both collection loop AND `parse()` as defense-in-depth (collection loop ensures bytes are never read beyond limit; parse() guard ensures no caller bypasses the loop)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed pre-existing compile failures in time/logger modules**
- **Found during:** Task 1 (first `cargo test` run)
- **Issue:** `src/time/mod.rs` tests referenced `civil_from_days()` function (not yet in git) and `DateTime::now()` with wrong signature; `src/time/error.rs` and `src/logger/mod.rs` had unstaged changes from prior work that were necessary for compilation. All tests failed to compile.
- **Fix:** Committed pre-existing unstaged changes (time/error.rs, logger/mod.rs), restored `civil_from_days` tests in time module, added `#[allow(dead_code)]` to future-use public APIs that triggered `-D dead-code` in clippy hook
- **Files modified:** src/time/mod.rs, src/time/error.rs, src/logger/mod.rs
- **Verification:** `cargo test` passes all 11 then 13 tests
- **Committed in:** `2cdef4a` (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (Rule 3 - blocking)
**Impact on plan:** Necessary to enable any test execution. No scope creep — the unstaged changes were pre-existing work that needed to be landed to unblock the build.

## Issues Encountered
- Pre-commit hooks run both `cargo fmt` and `cargo clippy -D warnings` — required passing clippy with no dead-code warnings before any commit could land. Future-use public APIs in `time/mod.rs` needed `#[allow(dead_code)]` annotations.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All five SAFE-03 through SAFE-06 crash vectors eliminated; server is safe to build on
- Phase 2 (Response layer) can proceed: `Error::RequestTooLarge` is ready to be mapped to `400 Bad Request` response
- Phase 3 (Routing) has a clean minimal routing stub to replace
- `RequestMethod` enum and `Request::parse()` are stable, tested interfaces

## Self-Check: PASSED

- FOUND: .planning/phases/01-foundation-fixes/01-02-SUMMARY.md
- FOUND: src/server/error.rs (contains RequestTooLarge)
- FOUND: src/server/request.rs (contains MAX_HEADER_LINES)
- FOUND: src/server/worker.rs (contains unwrap_or_else)
- FOUND: src/server/pool.rs (contains let _ = thread.join())
- FOUND: src/server/mod.rs (contains read_line)
- FOUND: commit 2cdef4a (feat(01-02): add RequestTooLarge error variant)
- FOUND: commit f631262 (fix(01-02): eliminate crash vectors)
- All 13 tests pass (cargo test exit 0)

---
*Phase: 01-foundation-fixes*
*Completed: 2026-03-01*
