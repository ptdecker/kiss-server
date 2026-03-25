# Phase 12: Badge, Docs, README - Context

**Gathered:** 2026-03-24
**Status:** Ready for planning

<domain>
## Phase Boundary

Create/replace four documentation artifacts: update README.md (full replacement), create docs/ci-cd.md, create docs/design.md (detailed architecture), and create scripts/README.md. Add a CI badge to README. No code changes.

Phase 12.1 (clean-up-in-aisle-9) handles code and file cleanup — this phase is docs only.

</domain>

<decisions>
## Implementation Decisions

### CI Badge (DOCS-01)
- **D-01:** Badge placed at the very top of README.md, before the `# title` heading — standard OSS convention, most visible placement
- **D-02:** Badge is a clickable link pointing to the GitHub Actions CI runs page (not a static image)

### docs/ci-cd.md (DOCS-02)
- **D-03:** Audience: dual-purpose — future-me quick reference AND external contributor onboarding (both equally)
- **D-04:** Four required sections:
  1. How CI works + triggers (what runs on push/PR, how to see results)
  2. How to deploy via prod branch (promote main → prod, what CD does, how to verify)
  3. How to check EC2 health (SSH commands, systemctl status)
  4. Setup from scratch (GitHub secrets, branch protection, AWS prerequisites)
- **D-05:** "Setup from scratch" section uses high-level steps + references to scripts — no copy-pasting script contents inline; the scripts directory README is the authoritative script reference

### README.md (DOCS-03)
- **D-06:** Full replacement — current content is too outdated (wrong title, old setup instructions, no CI/CD mention)
- **D-07:** Top-level README sections:
  1. CI badge (top, before title per D-01)
  2. What it is + core value (project description, no-external-deps philosophy)
  3. Build & run locally (`cargo build --release`, `--root` flag, example)
  4. Deployment overview (live at ptodd.org on EC2, how to deploy via prod, link to docs/ci-cd.md)
  5. Architecture (brief summary + link to docs/design.md)
  6. Scripts (brief mention + link to scripts/README.md)
- **D-08:** New `scripts/README.md` — describes each of the 8 scripts in scripts/, what each does and when to use it; top-level README references this file

### docs/design.md (new file)
- **D-09:** Detailed architecture walkthrough lives in `docs/design.md` (separate from README); covers Handler/Context/Router pattern, thread pool model, error handling approach, key design decisions
- **D-10:** README.md has a brief architecture summary (2-4 sentences) with a link to `docs/design.md` for deeper reading

### Claude's Discretion
- Exact badge markdown syntax (standard GitHub Actions badge format from ci.yml workflow name)
- Exact heading structure and prose within each doc
- Whether docs/design.md includes the key decisions table from PROJECT.md or a curated subset
- Tone and length of each section within the constraints above

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Acceptance Criteria
- `.planning/REQUIREMENTS.md` §DOCS — DOCS-01, DOCS-02, DOCS-03 are the acceptance criteria for this phase

### Existing CI/CD Workflows
- `.github/workflows/ci.yml` — workflow file name determines the badge URL; also the canonical reference for CI behavior description
- `.github/workflows/cd.yml` — canonical reference for CD pipeline description in docs/ci-cd.md

### Scripts
- `scripts/` directory — all 8 scripts to be documented in scripts/README.md:
  - `ci.sh` — local CI check runner
  - `install-kiss-server.sh` — installs binary + sets up swap, builds from source on EC2
  - `setup-aws-infra.sh` — provisions EC2, Security Group, Elastic IP
  - `setup-branch-protection.sh` — applies main branch protection ruleset via gh CLI
  - `setup-iptables.sh` — configures iptables port 80 → 8080 redirect on EC2
  - `setup-prod-protection.sh` — applies prod branch protection ruleset
  - `setup-webroot.sh` — creates /var/www/ptodd.org/ and deploys Hello World index.html
  - `verify-dns.sh` — smoke test for A record, CNAME, and HTTP content

### Project Context
- `.planning/PROJECT.md` — architecture principles, key decisions table, core value statement (source material for docs/design.md)
- `.planning/phases/11-cd-pipeline/11-CONTEXT.md` — deploy workflow specifics for docs/ci-cd.md

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- None — this phase creates documentation, not code

### Established Patterns
- All scripts follow: `#!/usr/bin/env bash`, `set -euo pipefail`, named constants, `echo "==> Step N:"` headers
- Repo name: `ptdecker/kiss-server` on GitHub

### Integration Points
- Badge URL format: `https://github.com/ptdecker/kiss-server/actions/workflows/ci.yml/badge.svg`
- Badge link target: `https://github.com/ptdecker/kiss-server/actions/workflows/ci.yml`
- EC2 SSH target: `ec2-user@54.83.192.65`
- Live site: `http://ptodd.org/`

</code_context>

<specifics>
## Specific Ideas

- docs/design.md should draw from the `Key Decisions` table in PROJECT.md — that's the authoritative list of architectural choices and their rationale
- scripts/README.md is a practical reference: what each script does, when to run it, any required prerequisites
- The deployment overview in README.md should mention the `git push origin origin/main:prod` pattern (the actual deploy command, from Phase 11 context)

</specifics>

<deferred>
## Deferred Ideas

- None — discussion stayed within phase scope

</deferred>

---

*Phase: 12-badge-docs-readme*
*Context gathered: 2026-03-24*
