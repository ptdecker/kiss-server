---
phase: 04-url-path-safety
plan: 01
subsystem: api
tags: [rust, url, percent-encoding, path-traversal, router, security]

# Dependency graph
requires:
  - phase: 03-handler-context-and-router
    provides: Router::dispatch() and Context/Request/Url types the new methods plug into

provides:
  - Url::path() — strips query string from raw_path
  - Url::query() — returns Option<&str> query portion
  - Url::decoded_path() — percent-decodes path using byte-buffer approach, returns Result<String>
  - Url::is_safe() — rejects dotdot components after decoding, returns bool
  - Router::dispatch() with safety guard rejecting invalid %-sequences and dotdot paths with 404
  - Decoded-path routing so percent-encoded requests match decoded route registrations (PATH-01)
  - Dotdot rejection at the router layer as defense-in-depth before Phase 5 filesystem check (PATH-02)

affects:
  - 05-static-file-handler (needs decoded_path and is_safe context; must implement PATH-03
    canonicalize + starts_with(root) as third traversal defense layer)

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "byte-buffer percent-decoding: iterate chars, on '%' consume two hex chars via hex_char_to_byte,
      push raw byte, then String::from_utf8 — avoids pct_decode multi-byte-per-call complexity"
    - "targeted #[allow(dead_code)] for forward-looking public API methods not yet consumed downstream"
    - "dispatch() uses decoded path for routing; rejection returns Ok(()) not Err (callers map Err to 500)"

key-files:
  created: []
  modified:
    - src/url/mod.rs
    - src/server/router.rs

key-decisions:
  - "decoded_path() uses byte-buffer approach (hex_char_to_byte directly) not pct_decode() — avoids multi-byte-per-call complexity; pct_decode handles one Unicode char at a time from arbitrary input, not sequential path bytes"
  - "dispatch() returns Ok(()) for all path rejection cases — never Err — because callers map Err to 500 and path rejection is a normal 404, not a server error"
  - "query() and is_safe() get targeted #[allow(dead_code)] — forward-looking public API consumed by Phase 5 StaticFileHandler; follows established project pattern from Phase 3"
  - "PATH-03 (canonicalize + starts_with(root)) explicitly deferred to Phase 5 StaticFileHandler — requires configured server root path that does not exist in this phase"

patterns-established:
  - "Defense-in-depth for path traversal: router layer rejects structural dotdot (PATH-01/02); Phase 5 must add canonicalize+prefix check (PATH-03) as third layer"

requirements-completed: [PATH-01, PATH-02]

# Metrics
duration: 5min
completed: 2026-03-02
---

# Phase 4 Plan 1: URL Path Safety Summary

**Percent-decode safety layer in Router::dispatch() rejects dotdot traversal and invalid %-sequences at the router boundary; Url gains path(), query(), decoded_path(), is_safe() methods**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-02T20:27:24Z
- **Completed:** 2026-03-02T20:32:34Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Added four public methods to `Url`: `path()`, `query()`, `decoded_path()`, `is_safe()` with 12 new unit tests
- Updated `Router::dispatch()` to percent-decode the target path before routing, enabling `/my%20file.html` to match a route registered as `/my file.html` (PATH-01)
- Router now rejects requests containing literal `..` or encoded `%2E%2E` components with 404 before any handler runs (PATH-02)
- Router rejects invalid %-sequences (e.g., `%GG`) with 404 before any handler runs
- Removed module-level `#![allow(unused)]` suppressor from `src/url/mod.rs`
- All 54 tests pass; `cargo clippy -- -D warnings` passes with zero warnings

## Task Commits

Each task was committed atomically:

1. **Task 1: Add path, query, decoded_path, is_safe to Url** - `be73643` (feat)
2. **Task 2: Update Router::dispatch() with safety guard** - `4d38f4e` (feat)

**Plan metadata:** (docs commit — see below)

_Note: Both tasks used TDD (RED then GREEN). Task 1 used temporary #[allow(dead_code)] during RED since methods weren't yet consumed; removed in Task 2 once router wired._

## Files Created/Modified
- `src/url/mod.rs` — Added `impl Url` block with `path()`, `query()`, `decoded_path()`, `is_safe()`; removed `#![allow(unused)]`; added 12 unit tests
- `src/server/router.rs` — Updated `dispatch()` with decoded-path safety guard; added 5 new router tests

## Decisions Made
- `decoded_path()` uses the byte-buffer approach calling `hex_char_to_byte()` directly rather than `pct_decode()` — `pct_decode()` processes one full Unicode character per call which introduces complexity when sequentially building a byte buffer from multi-byte sequences. The byte-buffer approach is simpler and correct.
- `dispatch()` returns `Ok(())` for all rejection cases — never `Err` — because `handle_connection` maps `Err` to HTTP 500. Path rejection is a normal 404 response, not a server error. `NotFoundHandler.handle()` already returns `Ok(())`.
- `query()` and `is_safe()` use targeted `#[allow(dead_code)]` — they are forward-looking public API consumed by Phase 5 `StaticFileHandler`. This follows the established project pattern from Phase 3.
- PATH-03 (`canonicalize` + `starts_with(root)` check) explicitly deferred to Phase 5 — it requires a configured server root path that does not exist in this phase. Phase 5 MUST implement this as the third traversal defense layer.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 2 - Missing Critical] Temporary targeted #[allow(dead_code)] during Task 1 commit**
- **Found during:** Task 1 (Url methods)
- **Issue:** Pre-commit hook runs `cargo clippy -D warnings`. After adding the four Url methods but before Task 2 wired `decoded_path()` into the router, all new methods triggered dead_code errors. The plan's done criteria require clippy to pass for each task commit, but the plan's own design requires two separate commits.
- **Fix:** Added `#[allow(dead_code)]` to the `impl Url` block and to `hex_char_to_byte` for Task 1's commit. These were removed in Task 2 once `decoded_path()` was consumed by `Router::dispatch()`. `query()` and `is_safe()` retain targeted `#[allow(dead_code)]` as forward-looking API (see Decisions Made).
- **Files modified:** `src/url/mod.rs`
- **Verification:** `cargo clippy -- -D warnings` passes at both task commits
- **Committed in:** `be73643` (Task 1), cleaned in `4d38f4e` (Task 2)

---

**Total deviations:** 1 auto-fixed (Rule 2 — missing critical: pre-commit hook compliance between sequenced TDD tasks)
**Impact on plan:** Necessary to maintain per-task atomic commits with clippy compliance. No scope creep. Final state matches plan intent exactly.

## Issues Encountered
None beyond the deviation documented above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Phase 5 (StaticFileHandler) can rely on `decoded_path()` and `is_safe()` from `src/url/mod.rs`
- Phase 5 MUST implement PATH-03: `canonicalize()` + `starts_with(root)` check — the third traversal defense layer requiring a configured server root path
- All three path-traversal defenses must be present as an atomic implementation in Phase 5

## Self-Check: PASSED

- FOUND: `src/url/mod.rs`
- FOUND: `src/server/router.rs`
- FOUND: `.planning/phases/04-url-path-safety/04-01-SUMMARY.md`
- FOUND commit `be73643`: feat(04-01): add path, query, decoded_path, is_safe to Url
- FOUND commit `4d38f4e`: feat(04-01): update Router::dispatch() with safety guard and decoded-path routing

---
*Phase: 04-url-path-safety*
*Completed: 2026-03-02*
