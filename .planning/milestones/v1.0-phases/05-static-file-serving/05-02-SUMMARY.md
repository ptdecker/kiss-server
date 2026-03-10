---
phase: 05-static-file-serving
plan: 02
subsystem: api
tags: [rust, static-files, mime-type, path-traversal, head-request, canonicalize]

# Dependency graph
requires:
  - phase: 05-static-file-serving/05-01
    provides: Router fallback slot (set_fallback() builder, fallback field in Router::dispatch)
  - phase: 04-url-path-safety
    provides: Url::decoded_path(), router dotdot component guard
provides:
  - StaticFileHandler struct with canonical_root: PathBuf field
  - StaticFileHandler::new(root: PathBuf) -> Result<Self> — canonicalizes root at construction
  - Handler impl for StaticFileHandler — full file serving with traversal guard
  - mime_type() free function — 10 locked extensions + application/octet-stream fallback
  - not_found() helper — 404 response with "Not Found" plain text body
affects:
  - 05-static-file-serving/05-03 — wires StaticFileHandler into main.rs via set_fallback()

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Forward-looking #[allow(dead_code)] pattern for pub APIs wired in a subsequent plan"
    - "canonicalize + starts_with(canonical_root) as final path traversal defense layer"
    - "Match on io::ErrorKind::NotFound from canonicalize() to return 404 not 500"
    - "Idempotent test setup: remove pre-existing symlinks before creating them"
    - "Error type boundary: map Box<dyn Error> from decoded_path() to 404 at handler boundary"

key-files:
  created: []
  modified:
    - src/handlers/mod.rs

key-decisions:
  - "decoded_path() Box<dyn Error> mapped to 404 at StaticFileHandler handler boundary — server::Error has no From<Box<dyn Error>> impl, and an invalid %-sequence is a malformed request (not a server error)"
  - "Symlink test uses remove_file before creating symlink to ensure idempotent test runs across multiple cargo test invocations"
  - "#[allow(dead_code)] on mime_type, not_found, StaticFileHandler, and StaticFileHandler::new — all consumed once main.rs is wired in Plan 05-03"

patterns-established:
  - "StaticFileHandler::handle() error boundary: all 404 conditions (missing file, traversal rejection, invalid path) return Ok(()) + 404 response; only unexpected I/O errors return Err (which becomes 500)"

requirements-completed: [FILE-01, FILE-02, FILE-04, FILE-05, PATH-03]

# Metrics
duration: 2min
completed: 2026-03-02
---

# Phase 5 Plan 02: StaticFileHandler Summary

**StaticFileHandler with binary-safe fs::read(), 10-type MIME detection, canonicalize+starts_with PATH-03 guard, and HEAD-only headers branch — 83 tests green**

## Performance

- **Duration:** ~2 min
- **Started:** 2026-03-02T21:19:13Z
- **Completed:** 2026-03-02T21:20:49Z
- **Tasks:** 1 (combined implementation with tests per TDD pattern)
- **Files modified:** 1

## Accomplishments

- StaticFileHandler implemented with canonical_root field computed once at construction
- mime_type() covers 10 locked extensions (html, css, js, wasm, png, jpg/jpeg, gif, svg, ico, txt) plus application/octet-stream fallback
- PATH-03 traversal guard: canonicalize(candidate).starts_with(canonical_root) catches symlink escapes
- HEAD vs GET branch: HEAD sets Content-Type/Content-Length/Connection headers only, no body bytes
- 27 new handler tests (13 MIME unit, 4 GET serving, 1 HEAD, 1 PATH-03 symlink, 2 constructor, 1 not_found); all 83 project tests green

## Task Commits

1. **StaticFileHandler TDD implementation** - `7e199c2` (feat)

**Plan metadata:** pending (docs commit)

## Files Created/Modified

- `/Users/todddecker/rust/ptodd/src/handlers/mod.rs` - Added mime_type(), not_found(), StaticFileHandler struct + new() + Handler impl, plus comprehensive test suite

## Decisions Made

- **decoded_path() error mapped to 404:** `Url::decoded_path()` returns `Result<String, Box<dyn Error>>` but `Handler::handle()` uses `server::Result<()>` = `Result<(), server::Error>`. No `From<Box<dyn Error>> for server::Error` exists. Mapping the error to 404 (not 500) is correct — an invalid %-sequence is a malformed request path, identical to how Router treats it.

- **Symlink test idempotency:** The symlink test used a fixed temp dir name (`ptodd_test_path03_symlink`). On a second `cargo test` run the directory and symlink already existed, causing `symlink()` to fail with EEXIST. Added `let _ = std::fs::remove_file(&link_path)` before creating the symlink.

- **Forward-looking dead_code suppression:** clippy -D warnings fired for all four items (`mime_type`, `not_found`, `StaticFileHandler`, `StaticFileHandler::new`) because `main.rs` doesn't yet construct `StaticFileHandler`. Applied targeted `#[allow(dead_code)]` on each item with a comment pointing to Plan 05-03. Same pattern established in Phase 3 and Phase 5 Plan 01.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Map decoded_path() type error to 404 instead of compile error**
- **Found during:** Task 1 (StaticFileHandler implementation)
- **Issue:** `ctx.request.target.decoded_path()?` failed to compile — `?` operator could not convert `Box<dyn std::error::Error>` to `server::Error` because `From<Box<dyn Error>>` is not implemented for `server::Error`
- **Fix:** Replaced `?` with explicit match: `Err(_) => return not_found(ctx)` — invalid %-sequences in request path return 404, not 500
- **Files modified:** src/handlers/mod.rs
- **Verification:** `cargo test` passes 83/83; `cargo clippy -- -D warnings` zero warnings
- **Committed in:** 7e199c2

**2. [Rule 1 - Bug] Fix symlink test to be idempotent across test runs**
- **Found during:** Task 1 (symlink_escaping_root_returns_404 test)
- **Issue:** Test used fixed temp dir name; on second `cargo test` invocation the symlink already existed, causing `std::os::unix::fs::symlink().unwrap()` to panic with EEXIST (code 17)
- **Fix:** Added `let _ = std::fs::remove_file(&link_path);` immediately before the symlink call
- **Files modified:** src/handlers/mod.rs
- **Verification:** Running `cargo test` twice in sequence: both pass
- **Committed in:** 7e199c2

---

**Total deviations:** 2 auto-fixed (both Rule 1 - Bug)
**Impact on plan:** Both fixes necessary for correctness. No scope creep. Core implementation unchanged from plan specification.

## Issues Encountered

- Pre-commit hook requires `cargo fmt` — code formatted automatically before final commit. No substantive changes.

## Next Phase Readiness

- StaticFileHandler is complete and ready to be registered in main.rs
- Plan 05-03 wires StaticFileHandler via `router.set_fallback(StaticFileHandler::new(root)?)` and adds `--root <path>` CLI argument parsing
- No blockers

## Self-Check: PASSED

- FOUND: .planning/phases/05-static-file-serving/05-02-SUMMARY.md
- FOUND: src/handlers/mod.rs
- FOUND: commit 7e199c2

---
*Phase: 05-static-file-serving*
*Completed: 2026-03-02*
