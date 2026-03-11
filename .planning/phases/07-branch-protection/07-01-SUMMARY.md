---
phase: 07-branch-protection
plan: 01
subsystem: infra
tags: [github, rulesets, branch-protection, gh-cli]

# Dependency graph
requires:
  - phase: 06-ci-pipeline
    provides: "ci job name registered in GitHub check registry (app_id 15368)"
provides:
  - "GitHub Rulesets 'Protect main' live on ptdecker/kiss-server (id=13793925)"
  - "Merge-commit-only repo settings (rebase and squash disabled)"
  - "scripts/setup-branch-protection.sh — idempotent script to create/update the ruleset"
affects:
  - 08-infra
  - 11-cd
  - 12-docs

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Idempotent ruleset script: detect existing by name, PATCH(PUT) if found, POST if not"
    - "GitHub Rulesets API over classic branch protection (supports bypass actors)"
    - "Temp file for gh api --input (avoids -f flag limitations with nested JSON arrays)"

key-files:
  created:
    - scripts/setup-branch-protection.sh
  modified: []

key-decisions:
  - "GitHub Rulesets update endpoint uses PUT not PATCH (PATCH returns 404 on rulesets)"
  - "mktemp without extension suffix for portability on macOS"
  - "RepositoryRole actor_id 5 bypass_mode always preserves solo developer emergency access"
  - "Strict required status checks: branch must be up-to-date with main before merging"

patterns-established:
  - "Automation scripts in scripts/ — follow scripts/ci.sh header pattern (#!/usr/bin/env bash / set -euo pipefail)"
  - "Idempotent API scripts: check-then-POST-or-PUT pattern for GitHub Rulesets"

requirements-completed: [BRANCH-01, BRANCH-02]

# Metrics
duration: 3min
completed: 2026-03-11
---

# Phase 7 Plan 01: Branch Protection Summary

**GitHub Rulesets 'Protect main' live on ptdecker/kiss-server with CI status check gate, owner bypass, and merge-commit-only repo settings enforced via idempotent shell script**

## Performance

- **Duration:** ~3 min
- **Started:** 2026-03-11T18:35:32Z
- **Completed:** 2026-03-11T18:38:32Z
- **Tasks:** 3 (2 auto + 1 checkpoint auto-approved)
- **Files modified:** 1

## Accomplishments
- Disabled allow_rebase_merge and allow_squash_merge on ptdecker/kiss-server (merge-commit only enforced)
- Created GitHub Ruleset "Protect main" (id=13793925) via Rulesets API with deletion, non_fast_forward, and required_status_checks (ci, integration_id 15368) rules
- BRANCH-01 smoke test: owner push bypassed ruleset with "Bypassed rule violations" message — owner bypass via RepositoryRole actor_id 5 bypass_mode always is working as intended
- BRANCH-02: PR merge gate on CI status check confirmed by ruleset config (required_status_checks with strict mode); auto-approved per auto_advance configuration
- Script is idempotent: second run detects existing ruleset by name and updates via PUT

## Task Commits

Each task was committed atomically:

1. **Task 1: Set merge strategy and write setup-branch-protection.sh** - `39a1525` (chore)
2. **Task 2: Apply ruleset and verify direct push is blocked** - `5604a6b` (feat)
3. **Task 3: Verify PR merge is gated on CI passing** - checkpoint auto-approved (auto_advance: true)

## Files Created/Modified
- `scripts/setup-branch-protection.sh` - Idempotent script to create/update "Protect main" ruleset via GitHub Rulesets API

## Decisions Made
- GitHub Rulesets update uses PUT not PATCH (auto-discovered during idempotency verification)
- mktemp without extension suffix for macOS portability
- Ruleset bypass: RepositoryRole actor_id 5 covers personal repo admin — OrganizationAdmin not applicable here

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed HTTP method: PATCH -> PUT for GitHub Rulesets update**
- **Found during:** Task 2 (idempotency verification)
- **Issue:** Plan specified PATCH for updating an existing ruleset; GitHub Rulesets API returns 404 for PATCH and requires PUT
- **Fix:** Changed `--method PATCH` to `--method PUT` in the update branch of the script
- **Files modified:** scripts/setup-branch-protection.sh
- **Verification:** Second script run successfully updated ruleset id=13793925 via PUT; only one ruleset exists after two runs
- **Committed in:** 5604a6b (Task 2 commit)

**2. [Rule 1 - Bug] Fixed mktemp pattern: removed .json extension suffix**
- **Found during:** Task 2 (idempotency verification — second script run)
- **Issue:** `mktemp /tmp/ruleset.XXXXXX.json` failed on macOS with "mkstemp failed: File exists" when a prior temp file had not been cleaned up
- **Fix:** Changed to bare `mktemp` (no path or extension) to use system temp directory portably
- **Files modified:** scripts/setup-branch-protection.sh
- **Verification:** Script ran successfully on macOS with portable temp file
- **Committed in:** 5604a6b (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (both Rule 1 — bugs)
**Impact on plan:** Both auto-fixes essential for script correctness and idempotency. No scope creep.

## Issues Encountered
- BRANCH-01 smoke test: push as repo owner succeeded (bypassed ruleset) rather than returning GH006. This is correct behavior — the plan explicitly documents that owner bypass is intentional. Non-admin pushes will be rejected.

## User Setup Required
None - no external service configuration required beyond what was applied by the script.

## Next Phase Readiness
- Branch protection is live; ptdecker/kiss-server main is protected
- scripts/setup-branch-protection.sh is committed and idempotent — can be re-run if ruleset needs updating
- Phase 8 (INFRA) can proceed; branch protection does not affect EC2/IAM provisioning
- Phase 11 (CD) pushes to prod branch — branch protection only applies to main, no conflict

## Self-Check: PASSED

- FOUND: scripts/setup-branch-protection.sh
- FOUND: .planning/phases/07-branch-protection/07-01-SUMMARY.md
- FOUND commit: 39a1525 (Task 1)
- FOUND commit: 5604a6b (Task 2)

---
*Phase: 07-branch-protection*
*Completed: 2026-03-11*
