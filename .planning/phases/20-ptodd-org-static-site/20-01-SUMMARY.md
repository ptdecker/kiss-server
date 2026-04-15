---
phase: 20-ptodd-org-static-site
plan: "01"
subsystem: infra
tags: [github-actions, scp, cloudfront, static-site, html, css, ssh]

# Dependency graph
requires:
  - phase: 18-multi-domain-virtual-hosting
    provides: ptodd.org vhost routing to /var/www/ptodd.org/ on EC2

provides:
  - ptodd-org-site GitHub repo with deploy.yml, index.html, style.css, README.md on initial-setup branch
  - CD workflow: push to main triggers SCP deploy to EC2 + CloudFront invalidation
  - Site-repo pattern documentation for replicating to new domains

affects:
  - future-domain-site-repos

# Tech tracking
tech-stack:
  added:
    - GitHub Actions (branch-based deploy workflow in separate repo)
    - Raw SCP CLI (file transfer without build step)
    - AWS CloudFront invalidation via AWS CLI
  patterns:
    - Site-repo pattern: separate GitHub repo per hosted domain, independent CI/CD
    - Explicit file list in scp (index.html style.css) prevents repo metadata in webroot
    - StrictHostKeyChecking=yes with pre-seeded EC2_KNOWN_HOSTS (no TOFU)
    - Post-deploy smoke test via curl HTTP 200 check

key-files:
  created:
    - ~/rust/ptodd-org-site/.github/workflows/deploy.yml
    - ~/rust/ptodd-org-site/index.html
    - ~/rust/ptodd-org-site/style.css
    - ~/rust/ptodd-org-site/README.md

key-decisions:
  - "Separate GitHub repo (ptodd-org-site) keeps site deploys independent from kiss-server"
  - "Branch-based trigger (push: branches: [main]) not semver tags — content sites are not versioned releases"
  - "Explicit scp file list (index.html style.css) prevents .github/ and README.md from landing in webroot"
  - "Initial push to initial-setup branch so secrets can be added before first deploy fires"
  - "Reused EC2_SSH_KEY and EC2_KNOWN_HOSTS from kiss-server pattern — same deploy key"

patterns-established:
  - "Site-repo pattern: one GitHub repo per hosted domain, copy deploy.yml updating scp path and smoke test URL"
  - "StrictHostKeyChecking=yes with pre-seeded known_hosts (no TOFU) for all SCP deploys"
  - "CloudFront invalidation on /* after every static file deploy"

requirements-completed:
  - SC-1
  - SC-2
  - SC-3
  - SC-4
  - SC-5

# Metrics
duration: 2min
completed: 2026-04-15
---

# Phase 20 Plan 01: ptodd-org-site Static Site Summary

**ptodd-org-site GitHub repo created with SCP deploy workflow, P. Todd Decker landing page (HTML/CSS), and site-repo pattern documentation — pushed to initial-setup branch, awaiting secrets configuration and first deploy**

## Performance

- **Duration:** ~2 min
- **Started:** 2026-04-15T18:39:59Z
- **Completed:** 2026-04-15T18:42:00Z
- **Tasks:** 3 of 4 complete (Task 4 is a human checkpoint)
- **Files modified:** 4 (all in new ptodd-org-site repo)

## Accomplishments

- Created ptodd-org-site GitHub repo at https://github.com/ptdecker/ptodd-org-site with all four files on `initial-setup` branch
- Implemented deploy.yml with SSH setup, explicit-file SCP, post-deploy smoke test, and CloudFront invalidation — mirroring kiss-server pattern
- Built professional P. Todd Decker landing page per UI-SPEC with system font, spacing scale, accessible color contrast, and 44px touch targets
- Documented site-repo pattern in README.md so future domains can replicate the setup in 6 steps

## Task Commits

Tasks 1-3 were committed atomically in the new ptodd-org-site repo:

1. **Tasks 1-3: All site files** - `484d65b` (Initial commit: ptodd.org landing page with deploy workflow)

Note: Tasks 1, 2, and 3 were committed together in the new repo's initial commit since all files needed to exist before `git init` was called. The new repo is not part of the kiss-server git tree.

**Task 4** is a `checkpoint:human-verify` — paused for user to add secrets and merge to main.

## Files Created/Modified

- `~/rust/ptodd-org-site/.github/workflows/deploy.yml` — CD workflow: checkout, SSH setup, SCP explicit-file deploy, HTTP smoke test, CloudFront invalidation
- `~/rust/ptodd-org-site/index.html` — P. Todd Decker landing page with name, one-liner, GitHub and email links
- `~/rust/ptodd-org-site/style.css` — Minimal stylesheet: CSS custom property spacing scale, system font, accessible colors (#1a1a1a/#0066cc on #ffffff), responsive breakpoint at 480px
- `~/rust/ptodd-org-site/README.md` — Site-repo replication pattern with 6-step guide for new domains

## Decisions Made

- Branch-based deploy trigger (`push: branches: [main]`) instead of semver tags — content sites don't need versioned releases
- Explicit `scp index.html style.css` (not `scp -r .`) to prevent `.github/` and `README.md` from landing in webroot (Pitfall 1 from RESEARCH.md)
- Pushed to `initial-setup` branch (not `main`) so deploy workflow cannot fire until user adds secrets and merges
- Reused same five secrets as kiss-server — `EC2_SSH_KEY`, `EC2_KNOWN_HOSTS`, `CF_AWS_ACCESS_KEY_ID`, `CF_AWS_SECRET_ACCESS_KEY`, `CLOUDFRONT_DISTRIBUTION_ID`

## Deviations from Plan

None — plan executed exactly as written.

## Known Stubs

None — all content is real: name, one-liner, GitHub handle, and email address from project context.

## Threat Flags

No new threat surface beyond what is documented in the plan's threat model (T-20-01 through T-20-06). All mitigations implemented:
- T-20-01: `StrictHostKeyChecking=yes` with `EC2_KNOWN_HOSTS` present in deploy.yml
- T-20-02: Explicit file list `index.html style.css` in scp command
- T-20-03: All credentials via `secrets.*` context, never hardcoded

## User Setup Required

**Five secrets must be added to the ptodd-org-site repo before the deploy workflow can succeed.**

Add each secret at: GitHub > ptdecker/ptodd-org-site > Settings > Secrets and variables > Actions > New repository secret

| Secret | Source |
|--------|--------|
| `EC2_SSH_KEY` | Same private key as kiss-server — copy from kiss-server repo Settings > Secrets > EC2_SSH_KEY, or from ~/.ssh/id_ed25519 |
| `EC2_KNOWN_HOSTS` | Same value as kiss-server — copy from kiss-server repo Settings > Secrets > EC2_KNOWN_HOSTS |
| `CF_AWS_ACCESS_KEY_ID` | Same IAM user kiss-cd-cloudfront — copy from kiss-server repo secrets |
| `CF_AWS_SECRET_ACCESS_KEY` | Same IAM user — copy from kiss-server repo secrets |
| `CLOUDFRONT_DISTRIBUTION_ID` | Value: E2JG60F8N1ZBAK (from docs/ci-cd.md) |

After adding secrets: review files on GitHub, confirm GitHub handle (ptdecker) and email (ptdecker@mac.com) are correct, create PR from `initial-setup` to `main`, merge to trigger first deploy, verify at https://www.ptodd.org/.

## Issues Encountered

None.

## Next Phase Readiness

- ptodd-org-site repo exists on GitHub at https://github.com/ptdecker/ptodd-org-site
- All four files (deploy.yml, index.html, style.css, README.md) are on `initial-setup` branch
- Task 4 (human checkpoint) is required: user must add 5 secrets and merge to main before site goes live
- After Task 4 completes: ptodd.org serves the P. Todd Decker landing page with working CD pipeline

---
*Phase: 20-ptodd-org-static-site*
*Completed: 2026-04-15*
