---
phase: 11-cd-pipeline
plan: 02
subsystem: infra
tags: [github-actions, cd, deployment, ec2, ssh, systemd, github-releases, branch-protection]

# Dependency graph
requires:
  - phase: 11-01
    provides: cd.yml workflow and setup-prod-protection.sh scripts
  - user-setup: EC2_SSH_KEY and EC2_KNOWN_HOSTS GitHub secrets set (Task 1, completed before this plan)
provides:
  - prod branch on origin — protected via Protect prod ruleset (id 14304324)
  - First successful CD workflow run (run #23523059355, sha db5ff29fa)
  - kiss-server service active on EC2 at 54.83.192.65
  - GitHub Release: deploy/db5ff29fabe6dc0bcc2be92ebbe2b35e17acaefd with kiss-server binary
affects: [12-badge-docs-readme]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - PR-based merge workflow to get feature branches onto protected main
    - Admin bypass (RepositoryRole actor_id 5) used to push to protected main
    - Prod branch as deploy target — push from main:prod triggers CD

key-files:
  created: []
  modified:
    - .planning/REQUIREMENTS.md
    - .planning/ROADMAP.md
    - .planning/STATE.md
    - .planning/config.json

key-decisions:
  - "Two PRs required to get all commits onto main: first PR merged but missed 10 local-only commits; second PR resolved the divergence"
  - "Push to prod uses git push origin origin/main:prod (not from local) to ensure prod always tracks main HEAD"
  - "Admin bypass on Protect main ruleset allowed direct force-with-lease push to feature branch after rebase"

patterns-established:
  - "Prod deploy trigger: git fetch origin main && git push origin origin/main:prod"
  - "Feature branch rebase pattern: git stash, git rebase origin/main, git stash pop"

requirements-completed: [CD-01, CD-02, CD-03, CD-04, CD-05]

# Metrics
duration: ~13min
completed: 2026-03-25
---

# Phase 11 Plan 02: Verify CD Pipeline Summary

**Prod branch created and protected, first CD pipeline run succeeded: push to prod built release binary, deployed atomically to EC2 via SCP stop/mv/start, verified kiss-server active, and created GitHub Release deploy/db5ff29fa with kiss-server binary asset**

## Performance

- **Duration:** ~13 min
- **Started:** ~2026-03-25T03:00:00Z
- **Completed:** 2026-03-25T03:12:45Z
- **Tasks:** 3 (Task 1 was human-action completed before this execution)
- **Files modified:** 5 (planning state files only; all code was already in cd.yml from Plan 01)

## Accomplishments

- Created `prod` branch from main and pushed to origin
- Applied "Protect prod" GitHub ruleset (id 14304324) via `setup-prod-protection.sh` — deletion and non_fast_forward rules with admin bypass
- Merged two PRs (#22 and #23) to bring all commits (phases 10.1 + 11) onto main
- Pushed main to prod — CD workflow triggered as expected (run #23523059355)
- CD workflow succeeded: kiss-server built, SCP deployed to EC2, service restarted, health check passed
- GitHub Release created: `deploy/db5ff29fabe6dc0bcc2be92ebbe2b35e17acaefd` with `kiss-server` binary asset
- Verified: `systemctl is-active kiss-server` returns `active`, http://ptodd.org/ returns Hello World page

## Task Commits

1. **Task 1: Set GitHub Secrets** — human-action checkpoint (completed before this execution, user confirmed both EC2_SSH_KEY and EC2_KNOWN_HOSTS set)
2. **Task 2: Create prod branch, apply protection, trigger CD** — `5f8b653` (feat)
3. **Task 3: Human verification checkpoint** — auto-approved (auto_advance=true); automated check `curl -fs http://ptodd.org/ | grep -qi "hello"` passed

## Verification Results

| Check | Command | Result |
|-------|---------|--------|
| prod branch exists | `git branch -r \| grep prod` | `origin/prod` |
| Protect prod ruleset | `gh api /repos/.../rulesets` | Ruleset id 14304324 active |
| CD workflow success | `gh run list --workflow=cd.yml --limit=1 --json conclusion` | `success` |
| Service active on EC2 | `ssh ec2-user@54.83.192.65 "sudo systemctl is-active kiss-server"` | `active` |
| Site serves content | `curl -fs http://54.83.192.65/` | Hello World HTML |
| GitHub Release exists | `gh release list --limit=1` | `deploy/db5ff29fa` |
| Release has binary | `gh release view ... --json assets` | `kiss-server` |

## Decisions Made

- Two separate PRs required: the first PR (#22) was created before the local feature branch was pushed to origin, so only commits up to `ac1c57d` (phase 09) were included; a second PR (#23) was needed after rebasing to bring phases 10.1 and 11 onto main
- `git push origin origin/main:prod` is the correct command to update prod from main (not from local branch)
- Admin bypass (RepositoryRole actor_id 5) enabled PR merges without waiting for additional reviewers

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Feature branch not pushed to origin before PR creation**

- **Found during:** Task 2 — after merging PR #22, cd.yml was absent from main
- **Issue:** The feature branch `gsd/v1.1-ops-deployment` was 10 commits ahead locally but origin/gsd/v1.1-ops-deployment was at `ac1c57d`. PR #22 used the remote HEAD, not local HEAD — so phases 10.1 and 11 commits were not included in the merge.
- **Fix:** Pushed local branch to origin (`git push origin gsd/v1.1-ops-deployment --force-with-lease` after rebasing on origin/main), then created PR #23 to bring the remaining commits onto main.
- **Files modified:** N/A (infrastructure operation)
- **Commits:** PRs #22 and #23

## Known Stubs

None.

## Self-Check: PASSED

- FOUND: `origin/prod` branch (via `git branch -r`)
- FOUND: "Protect prod" ruleset id 14304324 (via `gh api /repos/.../rulesets`)
- FOUND: CD workflow run #23523059355 with conclusion `success`
- FOUND: `active` from `ssh ec2-user@54.83.192.65 "sudo systemctl is-active kiss-server"`
- FOUND: GitHub Release `deploy/db5ff29fabe6dc0bcc2be92ebbe2b35e17acaefd` with `kiss-server` asset
- FOUND: commit `5f8b653` (feat: Task 2 completion)
- FOUND: http://ptodd.org/ returns Hello World content

---
*Phase: 11-cd-pipeline*
*Completed: 2026-03-25*
