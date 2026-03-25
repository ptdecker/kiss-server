---
phase: 09-ec2-service-setup
plan: 01
subsystem: infra
tags: [rust, cli, systemd, port-binding]

# Dependency graph
requires: []
provides:
  - "parse_port_from() function accepting --port <num> CLI flag"
  - "kiss-server binds to 0.0.0.0:<port> (was localhost:6502)"
  - "DEFAULT_PORT: u16 = 6502 backward-compatible default"
affects: [09-02, phase-11-cd-pipeline]

# Tech tracking
tech-stack:
  added: []
  patterns: [parse_root_from/parse_port_from parallel structure for CLI arg parsing]

key-files:
  created: []
  modified:
    - src/main.rs
    - src/server/mod.rs

key-decisions:
  - "parse_root() wrapper removed — main() calls parse_root_from(&args) directly to share args with parse_port_from()"
  - "log::debug and log::warn imports moved to server/mod.rs where macros are actually used; main.rs retains only log::info"

patterns-established:
  - "CLI arg parser pattern: fn parse_X_from(args: &[String]) -> crate::Result<T> with position-based scanning"

requirements-completed: [DEPLOY-03]

# Metrics
duration: 2min
completed: 2026-03-24
---

# Phase 9 Plan 01: EC2 Service Setup Summary

**parse_port_from() added to src/main.rs — kiss-server now accepts `--port <num>` and binds to `0.0.0.0:<port>`, enabling systemd service invocation with `--port 8080` and iptables PREROUTING redirect from port 80**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-24T20:51:23Z
- **Completed:** 2026-03-24T20:53:43Z
- **Tasks:** 1 (TDD: RED observed + GREEN committed)
- **Files modified:** 2

## Accomplishments
- Added `const DEFAULT_PORT: u16 = 6502` replacing `const DEFAULT_ADDR: &str = "localhost:6502"`
- Implemented `parse_port_from(args: &[String]) -> crate::Result<u16>` following exact `parse_root_from()` pattern
- Updated `main()` to call both parsers and bind to `format!("0.0.0.0:{}", port)`
- Three new unit tests: valid port returns Ok(8080), no flag returns Ok(6502), invalid value returns Err with "--port" in message
- All 86 tests pass (7 in main.rs, 79 in other modules)

## Task Commits

1. **Task 1: --port flag and 0.0.0.0 bind (TDD GREEN)** - `617efad` (feat)

_Note: TDD RED phase observed (compile error confirmed) but not committed separately — pre-commit hook requires `cargo test` to pass, so RED and GREEN were combined into a single passing commit._

## Files Created/Modified
- `src/main.rs` - Added `DEFAULT_PORT`, `parse_port_from()`, three unit tests; updated `main()` for port-based binding; removed `parse_root()` wrapper and unused log imports
- `src/server/mod.rs` - Added `use log::{debug, warn}` (moved from main.rs to module where macros are used)

## Decisions Made
- `parse_root()` removed: `main()` now calls `parse_root_from(&args)` directly so both parsers share the same args slice — cleaner and avoids double `std::env::args()` collection
- `debug`/`warn` log imports moved to `src/server/mod.rs`: they were only used in server submodules (via `use super::*` glob); main.rs only needs `info`

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Moved log macro imports to server/mod.rs**
- **Found during:** Task 1 (GREEN phase)
- **Issue:** Original `main.rs` imported `use log::{debug, info, warn}` and server submodules relied on them being in scope via `use super::*`. Removing unused imports from `main.rs` broke compilation of `server/mod.rs`, `server/worker.rs`, and `server/pool.rs`
- **Fix:** Added `use log::{debug, warn}` directly to `src/server/mod.rs` where the macros are actually called; `main.rs` retains only `use log::info`
- **Files modified:** `src/server/mod.rs`
- **Verification:** `cargo clippy --all-targets -- -D warnings` clean; all 86 tests pass
- **Committed in:** `617efad` (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (Rule 3 - blocking)
**Impact on plan:** Fix required for correct compilation and clippy compliance. No scope creep.

## Issues Encountered
- Pre-commit hook runs `cargo test` — could not make a RED-only commit. TDD RED phase was observed via compile failure (`cargo test` output confirmed) then immediately proceeded to GREEN. Documented above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- `kiss-server --root /var/www/ptodd.org --port 8080` now works as the systemd unit expects
- Binary builds cleanly: `cargo build --release` produces `target/release/ptodd`
- Phase 09-02 can proceed: write `install-kiss-server.sh`, `setup-webroot.sh`, `setup-iptables.sh` and deploy to EC2

## Self-Check: PASSED

- `src/main.rs` exists: FOUND
- `src/server/mod.rs` exists: FOUND
- `09-01-SUMMARY.md` exists: FOUND
- Commit `617efad` exists: FOUND
- `fn parse_port_from` in source: FOUND
- `format!("0.0.0.0:{}", port)` in source: FOUND
- `const DEFAULT_PORT` in source: FOUND
- `const DEFAULT_ADDR` removed: CONFIRMED GONE

---
*Phase: 09-ec2-service-setup*
*Completed: 2026-03-24*
