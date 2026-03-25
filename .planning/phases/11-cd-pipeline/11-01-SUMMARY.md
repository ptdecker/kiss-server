---
phase: 11-cd-pipeline
plan: 01
subsystem: infra
tags: [github-actions, cd, deployment, ec2, ssh, systemd, github-releases]

# Dependency graph
requires:
  - phase: 06-ci-pipeline
    provides: CI workflow pattern (checkout@v4, dtolnay/rust-toolchain, Swatinem/rust-cache) mirrored in CD
  - phase: 09-ec2-service-setup
    provides: systemd kiss-server service, /usr/local/bin/kiss-server install path, ec2-user SSH target
provides:
  - .github/workflows/cd.yml — GitHub Actions CD pipeline triggered on prod branch push
  - scripts/setup-prod-protection.sh — idempotent prod branch protection setup script
affects: [11-02-verify-pipeline, 12-badge-docs-readme]

# Tech tracking
tech-stack:
  added: [softprops/action-gh-release@v2]
  patterns:
    - Atomic binary deployment: SCP to /tmp staging, stop service, mv, start service
    - Static known_hosts via EC2_KNOWN_HOSTS secret (avoids runtime ssh-keyscan TOFU risk)
    - deploy/{sha} release tag format distinguishes deploy artifacts from semver tags
    - permissions.contents:write required for GitHub Release creation in Actions

key-files:
  created:
    - .github/workflows/cd.yml
    - scripts/setup-prod-protection.sh
  modified: []

key-decisions:
  - "Used softprops/action-gh-release@v2 over gh release create for atomic tag+asset creation"
  - "Static EC2_KNOWN_HOSTS secret over runtime ssh-keyscan to prevent TOFU spoofing"
  - "setup-prod-protection.sh uses rtk proxy gh api to match established project pattern from setup-branch-protection.sh"
  - "No required_status_checks on prod ruleset — prod is a deploy target not a PR merge gate"
  - "deploy/{sha} tag format distinguishes CD releases from any future semver releases"

patterns-established:
  - "Atomic deploy pattern: SCP to /tmp/kiss-server-new, stop, mv to /usr/local/bin, start"
  - "CD workflow mirrors CI workflow structure: checkout@v4, dtolnay/rust-toolchain 1.93.1, Swatinem/rust-cache@v2"

requirements-completed: [CD-01, CD-02, CD-03, CD-04, CD-05]

# Metrics
duration: 2min
completed: 2026-03-24
---

# Phase 11 Plan 01: CD Pipeline Summary

**GitHub Actions CD workflow triggering on prod branch push — builds release binary, deploys atomically to EC2 via SCP + stop/mv/start, verifies systemd health, and creates a tagged GitHub Release with the binary asset**

## Performance

- **Duration:** ~2 min
- **Started:** 2026-03-25T02:47:54Z
- **Completed:** 2026-03-25T02:49:23Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments

- Created `.github/workflows/cd.yml` implementing all 5 CD requirements: prod trigger, release build, atomic deploy, health check, GitHub Release
- Created `scripts/setup-prod-protection.sh` as an idempotent script protecting the prod branch from deletion and non-fast-forward pushes with admin bypass
- CD workflow mirrors the established CI pattern (same toolchain, cache, checkout actions)

## Task Commits

Each task was committed atomically:

1. **Task 1: Create CD workflow file** - `3e4feed` (feat)
2. **Task 2: Create prod branch protection script** - `bd5bc95` (feat)

**Plan metadata:** (docs commit follows)

## Files Created/Modified

- `.github/workflows/cd.yml` — CD pipeline: prod trigger, release build, SCP atomic deploy, health check, GitHub Release with binary
- `scripts/setup-prod-protection.sh` — Idempotent gh CLI script to protect the prod branch (deletion + non_fast_forward, admin bypass)

## Decisions Made

- Used `softprops/action-gh-release@v2` over `gh release create` CLI — research recommends it for atomic tag+asset creation in a single step
- Used static `EC2_KNOWN_HOSTS` secret over runtime `ssh-keyscan` — prevents TOFU (trust on first use) host spoofing risk in CI runners
- `setup-prod-protection.sh` uses `rtk proxy gh api` to match the pattern established in `setup-branch-protection.sh`
- No `required_status_checks` rule on the prod ruleset — prod is a deploy target, not a PR merge gate; CI gates belong on main
- Binary named `kiss-server` throughout (from Cargo package rename in phase 10.1)

## Deviations from Plan

None — plan executed exactly as written.

Note: Plan mentioned that `setup-prod-protection.sh` "should work with plain gh for portability" vs the existing `setup-branch-protection.sh` using `rtk proxy`. The established project pattern (rtk proxy) was followed for consistency. This is a minor style choice with no functional impact.

## Issues Encountered

None.

## User Setup Required

Before executing the CD pipeline, two GitHub Actions secrets must be added to the repository:

- `EC2_SSH_KEY` — the private SSH key for `ec2-user@54.83.192.65` (contents of `~/.ssh/id_ed25519`)
- `EC2_KNOWN_HOSTS` — the EC2 host's known_hosts entry (output of `ssh-keyscan -t ed25519 54.83.192.65`)

Additionally, the `prod` branch must be created and `setup-prod-protection.sh` run to apply branch protection.

## Next Phase Readiness

- `cd.yml` and `setup-prod-protection.sh` are ready to push to the remote branch
- Plan 11-02 covers: push to GitHub, create prod branch, add secrets, trigger first CD run, verify deploy and GitHub Release

## Self-Check: PASSED

- FOUND: `.github/workflows/cd.yml`
- FOUND: `scripts/setup-prod-protection.sh`
- FOUND: `.planning/phases/11-cd-pipeline/11-01-SUMMARY.md`
- FOUND: commit `3e4feed` (feat: CD workflow file)
- FOUND: commit `bd5bc95` (feat: prod branch protection script)
- FOUND: commit `0a7e384` (docs: plan metadata)

---
*Phase: 11-cd-pipeline*
*Completed: 2026-03-24*
