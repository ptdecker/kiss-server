# Phase 11: CD Pipeline - Context

**Gathered:** 2026-03-24
**Status:** Ready for planning

<domain>
## Phase Boundary

GitHub Actions CD workflow (`cd.yml`) that fires on every push to the `prod` branch — builds a release binary, deploys it atomically to EC2 via SCP → stop → mv → start, verifies the service is running, and creates a GitHub Release tagged with the commit SHA.

CI (Phase 6), EC2 target (Phase 8), and systemd service (Phase 9) are complete prerequisites. DNS (Phase 10) is live. No Rust code changes. No documentation (Phase 12).

</domain>

<decisions>
## Implementation Decisions

### SSH Credentials
- **D-01:** Use `~/.ssh/id_ed25519` — the same key used locally to SSH to `ec2-user@54.83.192.65`
- **D-02:** Store the private key as a GitHub Actions secret named `EC2_SSH_KEY`
- **D-03:** The CD workflow references it as `${{ secrets.EC2_SSH_KEY }}`
- **D-04:** Setup ssh-agent or write key to `~/.ssh/` in the workflow step; add EC2 host to known_hosts to avoid interactive prompt (Claude's discretion on exact mechanism — `ssh-keyscan` or hardcoded fingerprint)

### Workflow File Structure
- **D-05:** New separate file: `.github/workflows/cd.yml` — clean separation from CI, different trigger, different concern
- **D-06:** Trigger: `push` to `prod` branch only
- **D-07:** Deploy directly — no re-running CI checks. The `prod` branch is only promoted after CI passes on `main`; re-running tests wastes time

### prod Branch Workflow
- **D-08:** Deploys are triggered by merging a PR from `main` → `prod`
- **D-09:** `prod` branch has protection: restricted to admins only (no direct push from non-admins)
- **D-10:** Branch protection script (`setup-branch-protection.sh` pattern) should be extended or a new script created for `prod` protection — Claude's discretion

### Deploy Sequence (locked from requirements)
- **D-11:** Atomic deploy: SCP binary to `/tmp/kiss-server-new` on EC2, then `systemctl stop kiss-server`, `mv /tmp/kiss-server-new /usr/local/bin/kiss-server`, `systemctl start kiss-server` (CD-03)
- **D-12:** Health check: `systemctl is-active kiss-server` after start — fail the pipeline if not active (CD-04)

### GitHub Release
- **D-13:** Release tag format: `deploy/{sha}` — e.g. `deploy/abc1234` — clearly identifies deploy artifacts vs semver releases
- **D-14:** Binary asset name in the release: `kiss-server` (simple, matches the binary name)
- **D-15:** Release created with `gh release create` or `softprops/action-gh-release` — Claude's discretion on Action vs gh CLI

### Claude's Discretion
- Exact mechanism for SSH known_hosts setup (`ssh-keyscan` at runtime vs static fingerprint)
- Whether to use `appleboy/ssh-action` or raw SSH commands in workflow steps
- Exact binary staging path on EC2 (e.g. `/tmp/kiss-server-new`)
- Whether to create/extend a prod branch protection script or handle via `gh` CLI inline
- `action-gh-release` vs `gh release create` for the GitHub Release step
- Target architecture for `cargo build --release` on GitHub Actions runner (`ubuntu-latest` → `x86_64-unknown-linux-gnu` — same as CI)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Existing workflows and scripts
- `.github/workflows/ci.yml` — CI workflow pattern; CD should follow same conventions (checkout, rust-toolchain, cache)
- `scripts/ci.sh` — script pattern: `set -euo pipefail`, named constants, step headers
- `scripts/setup-branch-protection.sh` — reference for gh CLI branch protection scripting pattern

### Requirements
- `.planning/REQUIREMENTS.md` §CD — CD-01 through CD-05 are the acceptance criteria

### Prior phase context
- `.planning/phases/06-ci-pipeline/06-CONTEXT.md` — CI infrastructure this phase builds on
- `.planning/phases/09-ec2-service-setup/09-CONTEXT.md` — systemd service, EC2 user, SSH target

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `.github/workflows/ci.yml` — uses `dtolnay/rust-toolchain@master` (toolchain 1.93.1), `Swatinem/rust-cache@v2`, single job `ci`. CD should mirror the build setup.
- `scripts/setup-branch-protection.sh` — pattern for `gh` CLI branch protection setup; extend or clone for `prod` branch

### Established Patterns
- All scripts: `#!/usr/bin/env bash`, `set -euo pipefail`, NAMED_CONSTANTS at top, `echo "==> Step N:"` headers
- GitHub Actions: checkout@v4, dtolnay/rust-toolchain, Swatinem/rust-cache — established, reuse in CD
- EC2 SSH: `ec2-user@54.83.192.65`, key at `~/.ssh/id_ed25519` locally

### Integration Points
- `systemctl stop/start kiss-server` — Phase 9 systemd unit is the deploy target
- `/usr/local/bin/kiss-server` — installed binary path (from Phase 9)
- `--profile kiss` / `us-east-1` — AWS CLI context (not needed in CD, but relevant for context)
- GitHub repo: ptdecker/ptodd (inferred) — `gh release create` will target this

</code_context>

<specifics>
## Specific Ideas

- Phase 9 CONTEXT.md noted a deferred idea: "Once Phase 11 CD is shipping releases, update `install-kiss-server.sh` to curl the release binary instead of building on EC2." This phase makes that possible — filing a GitHub issue is a good post-phase follow-up.
- The `deploy/{sha}` tag format keeps releases clearly separate from any future semver tags

</specifics>

<deferred>
## Deferred Ideas

- None — discussion stayed within phase scope

</deferred>

---

*Phase: 11-cd-pipeline*
*Context gathered: 2026-03-24*
