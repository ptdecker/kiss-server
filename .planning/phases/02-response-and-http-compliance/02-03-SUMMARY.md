---
phase: 02-response-and-http-compliance
plan: 03
subsystem: api
tags: [rust, http, response, rfc9110, rfc9112, date-header, error-handling]

# Dependency graph
requires:
  - phase: 02-01
    provides: DateTime::now() and to_imf_fixdate() for the Date response header
  - phase: 02-02
    provides: Response struct with value-chaining builder and RFC 9112-compliant write_to

provides:
  - handle_connection using Response builder for all responses (200 OK and error paths)
  - send_error_response helper for 400/431 error paths with body and mandatory headers
  - Date header via DateTime::to_imf_fixdate() on every response
  - 400 Bad Request response on invalid UTF-8 or I/O error (SAFE-01)
  - 431 Request Header Fields Too Large response when MAX_HEADER_LINES exceeded
  - All Phase 2 HTTP compliance requirements satisfied end-to-end

affects:
  - 03-routing (handle_connection pattern ready for routing extension)
  - 04-content-type (extend response builder with MIME detection)
  - 05-static-file-handler (uses same Response pattern for file responses)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "BufReader borrow constraint: collect I/O errors as Option<io::Error> inside block, handle after block ends when BufReader is dropped"
    - "send_error_response best-effort: let _ = response.write_to(stream) on error paths"
    - "Date header best-effort: omit if DateTime::now() fails, never panic"

key-files:
  created: []
  modified:
    - src/server/mod.rs

key-decisions:
  - "Collect BufReader I/O error as Option<io::Error> rather than explicit drop() — idiomatic Rust, avoids unsafe borrow-splitting"
  - "send_error_response takes &mut TcpStream (not &mut impl Write) — keeps stream-specific methods available for future phases"
  - "Date header is best-effort in send_error_response — omit if clock fails rather than panic on error path"
  - "Removed unused RequestMethod import — was no longer needed after replacing match-on-route with Response builder"

patterns-established:
  - "BufReader scope pattern: collect errors as Option inside the BufReader block, handle all errors after block"
  - "Error response pattern: send_error_response then return Err — client always gets a response before connection closes"

requirements-completed: [SAFE-01, HTTP-01, HTTP-02, HTTP-03, HTTP-06]

# Metrics
duration: 2min
completed: 2026-03-02
---

# Phase 2 Plan 3: Wire Response and Date Header into handle_connection Summary

**send_error_response helper and refactored handle_connection using Response builder with Content-Type, Content-Length, Date (IMF-fixdate), and Connection headers — 400/431 on bad input instead of silent close**

## Performance

- **Duration:** ~2 min
- **Started:** 2026-03-02T00:23:58Z
- **Completed:** 2026-03-02T00:25:39Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments
- Refactored `handle_connection` to use `Response::new().header().body().write_to()` builder for 200 OK responses
- Added `send_error_response` private helper: sends 400/431 with body, Content-Type, Content-Length, Date, Connection headers
- Wired `DateTime::to_imf_fixdate()` into all response paths via Date header
- Changed BufReader error handling from silent `return Err` to `send_error_response` then `return Err` (SAFE-01 satisfied)
- Added `get_root_response_has_required_headers` integration test verifying all 5 mandatory headers in live response
- All 24 tests pass (was 23, +1 new integration test)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add send_error_response and refactor handle_connection** - `46dfe50` (feat)

**Plan metadata:** (docs commit follows)

## Files Created/Modified
- `src/server/mod.rs` - Added `send_error_response`, refactored `handle_connection`, added `get_root_response_has_required_headers` test, added `use crate::time::DateTime` and `use response::Response` imports, removed unused `RequestMethod` import

## Decisions Made
- Collected BufReader I/O error as `Option<std::io::Error>` inside the block rather than using explicit `drop(reader)`. After the block ends naturally, the borrow is released and the error is handled. This is idiomatic Rust; `drop()` is a code smell in this context.
- `send_error_response` takes `&mut TcpStream` instead of `&mut impl Write` per the plan note — stream-specific methods (e.g., `peer_addr()`) may be needed in future phases.
- Removed unused `RequestMethod` import that remained from the old `match (&request.method, ...)` routing block.

## Deviations from Plan

None — plan executed exactly as written. The only minor adjustment was choosing the idiomatic `Option<io::Error>` pattern for BufReader error collection (the plan explicitly offered this as the preferred alternative to `drop(reader)`).

## Issues Encountered
- Pre-commit hook ran `cargo fmt` check and rejected double-space comments (`// EOF` with two spaces became `// EOF` with one). Fixed by running `cargo fmt` before committing. This is expected behavior from the project's pre-commit hook.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Phase 2 HTTP compliance requirements fully satisfied: SAFE-01, HTTP-01, HTTP-02, HTTP-03, HTTP-04, HTTP-05, HTTP-06
- `handle_connection` is ready for Phase 3 routing extension — the Response builder pattern is established
- No blockers

---
*Phase: 02-response-and-http-compliance*
*Completed: 2026-03-02*

## Self-Check: PASSED

- FOUND: src/server/mod.rs
- FOUND: 02-03-SUMMARY.md
- FOUND: commit 46dfe50 (feat: wire Response and DateTime into handle_connection)
- All 24 tests pass: `test result: ok. 24 passed; 0 failed`
