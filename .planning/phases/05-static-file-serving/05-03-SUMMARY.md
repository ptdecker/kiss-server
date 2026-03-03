---
phase: 05-static-file-serving
plan: 03
subsystem: api
tags: [rust, static-files, cli, main, integration, fallback]

# Dependency graph
requires:
  - phase: 05-static-file-serving/05-01
    provides: Router::set_fallback() builder
  - phase: 05-static-file-serving/05-02
    provides: StaticFileHandler::new(root) + Handler impl
provides:
  - "parse_root_from(args: &[String]) — testable helper for --root CLI parsing with is_dir() validation"
  - "parse_root() — thin wrapper over parse_root_from using std::env::args().skip(1)"
  - "main() wired with StaticFileHandler as router fallback via router.set_fallback(handler)"
  - "Complete end-to-end file serving: start server with --root, serve files via StaticFileHandler"
affects: []

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Extract parse_root_from(args: &[String]) from parse_root() to enable unit testing without std::env::args() injection"
    - "parse_root() validates is_dir() before StaticFileHandler::new() canonicalizes — two-layer startup validation"

key-files:
  created: []
  modified:
    - src/main.rs

key-decisions:
  - "Extract parse_root_from(args: &[String]) helper so --root parsing logic is unit-testable without std::env::args() injection"
  - "parse_root() skips args[0] (binary name) via .skip(1) before collecting into Vec<String>"
  - "is_dir() check in parse_root_from() gives user-friendly error at startup; StaticFileHandler::new() then canonicalizes for symlink correctness"
  - "cargo fmt reformatted parse_root_from() line wrapping — no substantive change, accepted by pre-commit hook"

patterns-established:
  - "Testable CLI arg parsing: extract inner parse_X_from(args: &[String]) called by thin parse_X() wrapper"

requirements-completed: [FILE-03]

# Metrics
duration: 8min
completed: 2026-03-02
---

# Phase 5 Plan 03: --root CLI Wiring and main.rs Integration Summary

**parse_root_from() + parse_root() + StaticFileHandler fallback registration complete Phase 5: server starts with --root <dir>, serves files, 87 tests green**

## Performance

- **Duration:** ~8 min
- **Started:** 2026-03-02T21:28:09Z
- **Completed:** 2026-03-02T21:36:21Z
- **Tasks:** 1 (TDD: tests + implementation combined per pre-commit hook constraint)
- **Files modified:** 1

## Accomplishments

- parse_root_from(args: &[String]) extracted for testability — unit-testable without env injection
- parse_root() thin wrapper using std::env::args().skip(1) calls parse_root_from()
- main() updated: parse_root()? -> StaticFileHandler::new(root)? -> router.set_fallback(handler)
- 4 new unit tests covering valid dir, missing --root flag, --root with no path value, nonexistent path
- All 87 project tests pass; cargo clippy -- -D warnings clean; cargo build clean
- `cargo run` (no args) defaults --root to "." (current directory) and starts the server; --root is optional
- `cargo run -- --root /nonexistent` exits with descriptive error referencing --root

## Task Commits

Each task was committed atomically:

1. **Task 1: Wire --root CLI argument and StaticFileHandler into main.rs** - `4ef276f` (feat)
   - Combined implementation + tests per pre-commit hook constraint (TDD RED blocked by compilation requirement)

**Plan metadata:** pending (docs commit)

## Files Created/Modified

- `/Users/todddecker/rust/ptodd/src/main.rs` - Added parse_root_from(), parse_root(), updated main() with StaticFileHandler fallback registration; 4 new parse_root_from tests

## Decisions Made

- **Testable CLI parsing:** Extracted `parse_root_from(args: &[String])` inner function so the parsing logic can be unit tested without `std::env::args()`. `parse_root()` is the thin public-facing wrapper that collects `env::args().skip(1)` and delegates. This pattern allows the 4 unit tests to pass concrete arg slices directly.

- **args().skip(1):** The binary name at index 0 is skipped before collecting, so `parse_root_from` receives only the user-supplied arguments. This matches the RESEARCH.md pattern intent where `parse_root_from` skips `args[0]`.

- **Two-layer startup validation:** `is_dir()` in `parse_root_from` provides a user-friendly error at startup; `StaticFileHandler::new()` then canonicalizes to handle symlinks and relative paths. Both layers are needed as specified in the plan.

- **cargo fmt reformatting:** The pre-commit hook ran `cargo fmt`, which collapsed the multi-line `path_str` binding and `Err(...)` return in `parse_root_from` to single lines. No substantive change.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] cargo fmt required by pre-commit hook**
- **Found during:** Task 1 (first commit attempt)
- **Issue:** Pre-commit hook ran `cargo fmt` and found two formatting differences in `parse_root_from` (multi-line method chain for `.get(pos + 1)...ok_or()` and multi-line `return Err(...)`)
- **Fix:** Ran `cargo fmt` before re-staging and committing
- **Files modified:** src/main.rs (formatting only, no substantive changes)
- **Verification:** Pre-commit hook passed on second attempt; all 87 tests still pass
- **Committed in:** 4ef276f

---

**Total deviations:** 1 auto-fixed (Rule 3 - Blocking, formatting)
**Impact on plan:** Trivial formatting adjustment; no substantive code changes.

## Issues Encountered

- Pre-commit hook (`cargo fmt`) blocked first commit attempt — same pattern seen in Plans 05-01 and 05-02. Resolved by running `cargo fmt` and re-staging.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Phase 5 complete: all three plans executed, all FILE-* and PATH-03 requirements satisfied
- Server is end-to-end functional: starts with `--root <dir>`, serves static files with correct MIME types, handles HEAD requests, rejects path traversal, returns 404 for missing files
- Awaiting human verification checkpoint (Task 2): start server with `--root .`, test file serving, HEAD, 404, and GET / route

## Self-Check: PASSED

- FOUND: src/main.rs
- FOUND: .planning/phases/05-static-file-serving/05-03-SUMMARY.md
- FOUND commit: 4ef276f

---
*Phase: 05-static-file-serving*
*Completed: 2026-03-02*
