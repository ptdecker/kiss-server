---
phase: 03-handler-context-and-router
plan: 01
subsystem: api
tags: [rust, http, request, response, visibility]

# Dependency graph
requires:
  - phase: 02-response-and-http-compliance
    provides: Response struct with write_to and builder API
provides:
  - pub RequestMethod with PartialEq derive for == comparison in router dispatch
  - pub Request struct with pub method and pub target fields accessible outside server/
  - pub Request::parse accessible outside server/ module
  - Response::add_header(&mut self) for post-dispatch header injection

affects:
  - 03-02 (router: needs RequestMethod == comparison)
  - 03-03 (server mod: needs ctx.response.add_header for Date header)

# Tech tracking
tech-stack:
  added: []
  patterns: [mutating add_header alongside value-chaining builder, pub(super) preserved for internal-only constants]

key-files:
  created: []
  modified:
    - src/server/request.rs
    - src/server/response.rs

key-decisions:
  - "Request::parse promoted to pub for consistency even though server/mod.rs is in same module — avoids mixed visibility that would confuse future refactors"
  - "MAX_HEADER_LINES stays pub(super) — it is an internal server boundary constant, not part of the public API"
  - "add_header uses &mut self (mutating) while header() uses self (value-chaining) — two patterns coexist: builder for initial construction, add_header for post-dispatch injection"

patterns-established:
  - "Visibility: pub for types crossing module boundary, pub(super) for module-internal constants"
  - "PartialEq on enums used for dispatch comparison (not pattern matching) to keep router code natural"
  - "Mutating add_header enables post-dispatch header injection without reconstructing the response"

requirements-completed: [ROUT-01, ROUT-02, ROUT-04]

# Metrics
duration: 1min
completed: 2026-03-02
---

# Phase 3 Plan 01: Handler Context and Router - Prerequisites Summary

**Promoted Request/RequestMethod to pub with PartialEq and added Response::add_header for post-dispatch header injection, unblocking Plans 02 and 03**

## Performance

- **Duration:** 1 min
- **Started:** 2026-03-02T16:29:34Z
- **Completed:** 2026-03-02T16:30:49Z
- **Tasks:** 1
- **Files modified:** 2

## Accomplishments
- RequestMethod derives PartialEq — router dispatch can use `==` comparison without pattern matching
- Request, RequestMethod, and their fields promoted from pub(super) to pub — accessible from handlers module outside server/
- Response::add_header(&mut self) added — handle_connection can inject Date header after dispatch without rebuilding the response
- All 25 tests pass (24 original + 1 new add_header_appends_header)

## Task Commits

Each task was committed atomically:

1. **Task 1: Promote visibility on Request/RequestMethod and add Response::add_header** - `da3278d` (feat)

**Plan metadata:** (docs commit — see below)

_Note: TDD tasks may have multiple commits (test → feat → refactor). Pre-commit hook (cargo clippy + cargo test) blocked RED commit; RED was verified via manual test run, GREEN implemented and committed atomically._

## Files Created/Modified
- `src/server/request.rs` - RequestMethod: pub + PartialEq; Request struct and fields: pub; Request::parse: pub
- `src/server/response.rs` - Added add_header(&mut self) mutating method + add_header_appends_header test

## Decisions Made
- `Request::parse` promoted to `pub` for consistency even though `server/mod.rs` is in the same module — avoids mixed visibility that would confuse future refactors
- `MAX_HEADER_LINES` stays `pub(super)` — internal server boundary constant, not part of the public API
- `add_header` uses `&mut self` (mutating) while `header()` uses `self` (value-chaining) — two patterns coexist: builder for initial construction, `add_header` for post-dispatch injection

## Deviations from Plan

None - plan executed exactly as written.

Note: The pre-commit hook (cargo clippy + cargo fmt + cargo test) prevented committing a RED-state test in isolation since the code wouldn't compile with an unknown method reference. The RED state was verified via `cargo test` output confirming the compile error (`no method named 'add_header' found`), then GREEN was implemented and committed in a single atomic commit as required by the hook.

## Issues Encountered
- Pre-commit hook runs cargo clippy and cargo test, which means a TDD RED commit (test-only, no implementation) cannot be committed while the code fails to compile. Verified RED state manually, proceeded directly to GREEN implementation and committed atomically.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Plan 02 (Router): RequestMethod has PartialEq — `route_method == ctx.request.method` compiles
- Plan 03 (server mod): `ctx.response.add_header("Date", ...)` compiles for post-dispatch Date injection
- No blockers for Phase 3 Plans 02 and 03

---
*Phase: 03-handler-context-and-router*
*Completed: 2026-03-02*
