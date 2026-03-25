# Phase 11: CD Pipeline - Research

**Researched:** 2026-03-24
**Domain:** GitHub Actions CD workflow, SSH/SCP deployment, GitHub Releases
**Confidence:** HIGH

## Summary

This phase creates a single `.github/workflows/cd.yml` that triggers on pushes to the `prod` branch, builds a release binary from the existing Rust codebase, deploys it atomically to EC2 via SCP + SSH, verifies the service is healthy, and publishes a GitHub Release. All major decisions were locked during the discussion phase. Research confirms the recommended Action versions, validates the critical path quirks (binary name, SCP directory tree, known_hosts setup), and resolves two flagged blockers from STATE.md.

The Cargo package name is `ptodd`, so `cargo build --release` produces `target/release/ptodd` — not `target/release/kiss-server`. The workflow must rename or reference the binary by its actual name when attaching it to the release or SCPing it to EC2. This is the most important implementation pitfall.

The `prod` branch does not yet exist. It must be created before branch protection can be applied to it, and before any CD trigger will fire.

**Primary recommendation:** Use raw SSH commands (`ssh -i` and `scp`) inside `run:` steps rather than appleboy actions. This avoids the scp-action path-flattening complexity (strip_components), eliminates extra dependencies, and keeps the workflow readable. The CI pattern already uses script delegation; the CD workflow should follow the same approach.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- D-01: Use `~/.ssh/id_ed25519` — the same key used locally to SSH to `ec2-user@54.83.192.65`
- D-02: Store the private key as a GitHub Actions secret named `EC2_SSH_KEY`
- D-03: The CD workflow references it as `${{ secrets.EC2_SSH_KEY }}`
- D-04: Setup ssh-agent or write key to `~/.ssh/` in the workflow step; add EC2 host to known_hosts to avoid interactive prompt (Claude's discretion on exact mechanism — `ssh-keyscan` or hardcoded fingerprint)
- D-05: New separate file: `.github/workflows/cd.yml` — clean separation from CI, different trigger, different concern
- D-06: Trigger: `push` to `prod` branch only
- D-07: Deploy directly — no re-running CI checks. The `prod` branch is only promoted after CI passes on `main`; re-running tests wastes time
- D-08: Deploys are triggered by merging a PR from `main` → `prod`
- D-09: `prod` branch has protection: restricted to admins only (no direct push from non-admins)
- D-10: Branch protection script (`setup-branch-protection.sh` pattern) should be extended or a new script created for `prod` protection — Claude's discretion
- D-11: Atomic deploy: SCP binary to `/tmp/kiss-server-new` on EC2, then `systemctl stop kiss-server`, `mv /tmp/kiss-server-new /usr/local/bin/kiss-server`, `systemctl start kiss-server` (CD-03)
- D-12: Health check: `systemctl is-active kiss-server` after start — fail the pipeline if not active (CD-04)
- D-13: Release tag format: `deploy/{sha}` — e.g. `deploy/abc1234` — clearly identifies deploy artifacts vs semver releases
- D-14: Binary asset name in the release: `kiss-server` (simple, matches the binary name)
- D-15: Release created with `gh release create` or `softprops/action-gh-release` — Claude's discretion on Action vs gh CLI

### Claude's Discretion
- Exact mechanism for SSH known_hosts setup (`ssh-keyscan` at runtime vs static fingerprint)
- Whether to use `appleboy/ssh-action` or raw SSH commands in workflow steps
- Exact binary staging path on EC2 (e.g. `/tmp/kiss-server-new`)
- Whether to create/extend a prod branch protection script or handle via `gh` CLI inline
- `action-gh-release` vs `gh release create` for the GitHub Release step
- Target architecture for `cargo build --release` on GitHub Actions runner (`ubuntu-latest` → `x86_64-unknown-linux-gnu` — same as CI)

### Deferred Ideas (OUT OF SCOPE)
- None — discussion stayed within phase scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CD-01 | Pushing to the `prod` branch automatically triggers a deployment to EC2 | Workflow `on: push: branches: [prod]` trigger; `prod` branch must be created first |
| CD-02 | CD pipeline builds a release binary (`cargo build --release`) on the CI runner | Same toolchain/cache Actions as CI; binary produces `target/release/ptodd` (Cargo package name) |
| CD-03 | CD pipeline atomically replaces the binary on EC2 (SCP to temp → stop → mv → start) | Raw `scp` + `ssh` steps; avoid scp-action path-tree issue; binary staged at `/tmp/kiss-server-new` |
| CD-04 | CD pipeline verifies the service is running after deploy and fails the pipeline if not | `systemctl is-active kiss-server` exits 0 if active, non-zero otherwise; `set -e` propagates failure |
| CD-05 | CD pipeline creates a GitHub Release tagged with the prod commit SHA and attaches the compiled binary | `softprops/action-gh-release@v2` with `tag_name: deploy/${{ github.sha }}` and `files: target/release/ptodd` |
</phase_requirements>

## Standard Stack

### Core
| Library/Tool | Version | Purpose | Why Standard |
|-------------|---------|---------|--------------|
| actions/checkout | v4 | Checkout source at prod SHA | Same as CI; established in project |
| dtolnay/rust-toolchain | @master (pins 1.93.1) | Rust toolchain with exact version | Same as CI; pinned in rust-toolchain.toml |
| Swatinem/rust-cache | v2 | Cargo registry + build cache | Same as CI; reduces build time |
| softprops/action-gh-release | v2 | Create GitHub Release with asset | Current version (v2.6.1); supports custom tag_name required for `deploy/{sha}` format |
| ssh (native runner tool) | OpenSSH (pre-installed on ubuntu-latest) | SCP file transfer + remote SSH commands | Avoids appleboy/scp-action path-tree complexity; simpler for single-file deploy |

### Supporting
| Tool | Version | Purpose | When to Use |
|------|---------|---------|-------------|
| appleboy/ssh-action | v1 | Alternative for remote SSH steps | Only if raw SSH becomes awkward; adds external dependency for little gain |
| appleboy/scp-action | v1 | Alternative for SCP file transfer | Avoid — see Pitfall 1 (path tree reproduction) |
| gh CLI | pre-installed | Alternative for release creation | Use `softprops/action-gh-release@v2` instead; avoids needing `gh auth` in workflow |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Raw `scp`/`ssh` run steps | appleboy/scp-action + appleboy/ssh-action | Actions add external dependencies and the scp-action path behavior requires `strip_components` tuning; raw SSH is simpler for a single binary |
| softprops/action-gh-release | `gh release create` CLI | Both work; action is more idiomatic for workflows, handles tag creation atomically with asset upload |
| `ssh-keyscan` at runtime | Static fingerprint stored as secret | Runtime keyscan is slightly susceptible to MITM on first scan; static fingerprint captured now is more secure |

**Installation:** No npm installs — all Actions are used via `uses:` syntax in the workflow file.

## Architecture Patterns

### Recommended Workflow Structure

```
.github/workflows/
├── ci.yml        # Existing — push/PR to main
└── cd.yml        # New — push to prod only
scripts/
├── setup-branch-protection.sh    # Existing — main branch
└── setup-prod-protection.sh      # New — prod branch (admin-only)
```

### Pattern 1: Workflow Trigger for prod Branch

**What:** Trigger CD only on push to `prod`; no path filters needed since entire repo constitutes the service.
**When to use:** All deployments.

```yaml
# Source: GitHub Actions docs — workflow syntax
on:
  push:
    branches: [prod]
```

### Pattern 2: SSH Key Setup with Static Known Hosts

**What:** Write the SSH private key to a temp file, set permissions, add pre-captured EC2 host key to known_hosts. Static fingerprint is more secure than runtime `ssh-keyscan` (no MITM window).
**When to use:** Any step requiring SSH or SCP to EC2.

```yaml
- name: Setup SSH
  run: |
    mkdir -p ~/.ssh
    echo "${{ secrets.EC2_SSH_KEY }}" > ~/.ssh/deploy_key
    chmod 600 ~/.ssh/deploy_key
    echo "${{ secrets.EC2_KNOWN_HOSTS }}" >> ~/.ssh/known_hosts
```

The EC2 host key (`EC2_KNOWN_HOSTS`) can be stored as a second GitHub secret. The current EC2 host keys (retrieved 2026-03-24 via `ssh-keyscan -H 54.83.192.65`):
- `ecdsa-sha2-nistp256` key is available
- `ssh-ed25519` key is available

Store the `ssh-ed25519` line as `EC2_KNOWN_HOSTS` — it matches the key type of the deploy key. The hashed format (`|1|...`) prevents the IP from being visible in the secret value.

### Pattern 3: Build Release Binary

**What:** Mirror CI's toolchain/cache setup, then build release binary.
**When to use:** CD build step.

```yaml
- uses: actions/checkout@v4
- uses: dtolnay/rust-toolchain@master
  with:
    toolchain: "1.93.1"
- uses: Swatinem/rust-cache@v2
  with:
    cache-on-failure: "true"
- name: Build release binary
  run: cargo build --release --locked
```

The binary path after build: `target/release/ptodd` (Cargo package name is `ptodd`, not `kiss-server`).

### Pattern 4: Atomic Deploy via SCP + SSH

**What:** Copy binary to `/tmp/`, stop service, replace binary, start service. Never write directly over a running binary (SIGBUS risk).
**When to use:** Deploy step.

```yaml
- name: Deploy to EC2
  run: |
    scp -i ~/.ssh/deploy_key -o StrictHostKeyChecking=yes \
      target/release/ptodd \
      ec2-user@54.83.192.65:/tmp/kiss-server-new
    ssh -i ~/.ssh/deploy_key ec2-user@54.83.192.65 \
      "sudo systemctl stop kiss-server && \
       sudo mv /tmp/kiss-server-new /usr/local/bin/kiss-server && \
       sudo chmod +x /usr/local/bin/kiss-server && \
       sudo systemctl start kiss-server"
```

### Pattern 5: Health Check — Fail Pipeline if Service Down

**What:** `systemctl is-active` returns 0 if active, non-zero if inactive/failed. This is the correct check — `systemctl start` exits 0 even if the service crashes immediately after start.
**When to use:** After the deploy step, as a separate step so failure is clearly attributed.

```yaml
- name: Verify service is active
  run: |
    ssh -i ~/.ssh/deploy_key ec2-user@54.83.192.65 \
      "sudo systemctl is-active kiss-server"
```

### Pattern 6: GitHub Release with Custom Tag

**What:** Create a release tagged `deploy/{sha}` with the binary attached. Must specify `tag_name` explicitly because the workflow is triggered by a branch push, not a tag push — the default `github.ref_name` would produce `prod`, not the desired tag.
**When to use:** Final step of CD workflow.

```yaml
- name: Create GitHub Release
  uses: softprops/action-gh-release@v2
  with:
    tag_name: deploy/${{ github.sha }}
    name: "Deploy ${{ github.sha }}"
    files: target/release/ptodd
  env:
    GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

The action requires `permissions: contents: write` at the job or workflow level.

### Pattern 7: prod Branch Protection Script

**What:** Clone the existing `setup-branch-protection.sh` pattern, targeting `prod` with admin-bypass-only ruleset. No required status checks on prod (CD workflow is the only thing that pushes there, and it's triggered by the push itself).
**When to use:** One-time setup step, run locally, not in the workflow.

```bash
# setup-prod-protection.sh — protect prod from non-admin direct pushes
# Ruleset: deletion blocked, non-fast-forward blocked,
# bypass: RepositoryRole actor_id 5 (admin), bypass_mode: always
# No required status checks (prod is a deploy-only branch)
```

### Anti-Patterns to Avoid

- **Using `appleboy/scp-action` without `strip_components`:** The action uses tar internally and reproduces the full source directory tree under target. `source: target/release/ptodd` with `target: /tmp` produces `/tmp/target/release/ptodd`, not `/tmp/ptodd`. Requires `strip_components: 2` to get flat layout. Raw `scp` is simpler.
- **Skipping `chmod +x` after `mv`:** The `mv` preserves permissions if both paths are on the same filesystem, but it is safer to always set explicitly after replace.
- **Using `systemctl restart` for health verification:** `restart` exits 0 even when the service crashes immediately. Always use `is-active` as a separate check.
- **Assuming `cargo build --release` produces `kiss-server`:** The binary name comes from `[package] name` in `Cargo.toml`, which is `ptodd`. The binary is `target/release/ptodd`.
- **Using `github.ref_name` as release tag on a branch push:** On a `push` to `prod`, `github.ref_name` is `prod` — not the commit SHA. Must use `github.sha` to build the `deploy/{sha}` tag.
- **Running `ssh-keyscan` at workflow runtime without capturing the key first:** Susceptible to MITM during the first scan. Capture the known host key now (done — see Pattern 2) and store as a secret.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| GitHub Release creation | Custom gh CLI auth + release + upload sequence | `softprops/action-gh-release@v2` | Action handles tag creation, asset upload atomically; handles re-runs gracefully |
| SSH key management | `echo > file && chmod` spread across steps | Consolidated Setup SSH step (Pattern 2) | Permissions mistake leaves key world-readable; isolate in one step |

**Key insight:** The deploy itself is simple enough (stop/mv/start) that raw shell commands outperform action wrappers — fewer moving parts, easier to read in logs, no action version pinning for the critical path steps.

## Common Pitfalls

### Pitfall 1: scp-action Reproduces Source Directory Tree
**What goes wrong:** Using `appleboy/scp-action` with `source: target/release/ptodd` and `target: /tmp` deposits the file at `/tmp/target/release/ptodd` (not `/tmp/ptodd`), so the subsequent `mv /tmp/kiss-server-new` fails.
**Why it happens:** The action uses `tar` to bundle files and preserves relative paths during extraction.
**How to avoid:** Use raw `scp` in a `run:` step. If scp-action is used, set `strip_components: 2` to strip `target/release/` prefix.
**Warning signs:** Deploy step passes but `mv` fails with "No such file or directory".

### Pitfall 2: Binary Name is `ptodd`, Not `kiss-server`
**What goes wrong:** Workflow references `target/release/kiss-server` which does not exist. Build succeeds silently; SCP or release attachment step fails.
**Why it happens:** Cargo derives binary name from `[package] name = "ptodd"` in `Cargo.toml`. The service is named `kiss-server` but the binary is not.
**How to avoid:** Always reference `target/release/ptodd`. Rename to `kiss-server` only happens at install time (`/usr/local/bin/kiss-server`).
**Warning signs:** `scp: target/release/kiss-server: No such file or directory`

### Pitfall 3: Release Tag Requires Explicit `tag_name`
**What goes wrong:** `softprops/action-gh-release@v2` without `tag_name` uses `github.ref_name` which is `prod` on a branch push. Creates release tagged `prod` instead of `deploy/abc1234`, and fails on repeat deploys because the tag already exists.
**Why it happens:** The action defaults to the current git ref; for tag pushes this is correct, but for branch pushes it is not.
**How to avoid:** Always set `tag_name: deploy/${{ github.sha }}` explicitly (Pattern 6).
**Warning signs:** First deploy creates tag `prod`; second deploy fails with tag conflict.

### Pitfall 4: `prod` Branch Does Not Exist Yet
**What goes wrong:** Creating `cd.yml` with `on: push: branches: [prod]` has no effect until `prod` exists. Branch protection also cannot target a non-existent branch.
**Why it happens:** `prod` is only in planning docs, not in the repo.
**How to avoid:** Wave 0 task must create the `prod` branch from `main`, push it, then set up branch protection.
**Warning signs:** Workflow exists but never triggers.

### Pitfall 5: `GITHUB_TOKEN` Needs `contents: write` Permission for Release
**What goes wrong:** `softprops/action-gh-release` fails with "403 Resource not accessible by integration" if the job does not have write permission on contents.
**Why it happens:** GitHub Actions workflows default to read-only `GITHUB_TOKEN` in many organizations.
**How to avoid:** Add `permissions: contents: write` to the job block.
**Warning signs:** Release step fails with HTTP 403.

### Pitfall 6: `systemctl` Requires sudo on EC2
**What goes wrong:** SSH commands like `systemctl stop kiss-server` fail with "permission denied" because `ec2-user` is not root.
**Why it happens:** The kiss-server service was installed as a system service owned by root.
**How to avoid:** Prefix all systemctl commands with `sudo` in SSH one-liners. This is already established in Phase 9 scripts.
**Warning signs:** SSH step exits non-zero; `sudo: systemctl: command not found` would indicate PATH issue.

## Code Examples

Verified patterns from official sources and project conventions:

### Complete cd.yml Skeleton (Reference)
```yaml
# Source: GitHub Actions workflow syntax docs + project CI pattern
name: CD

on:
  push:
    branches: [prod]

env:
  CARGO_TERM_COLOR: always

jobs:
  deploy:
    runs-on: ubuntu-latest
    permissions:
      contents: write

    steps:
      - uses: actions/checkout@v4

      - uses: dtolnay/rust-toolchain@master
        with:
          toolchain: "1.93.1"

      - uses: Swatinem/rust-cache@v2
        with:
          cache-on-failure: "true"

      - name: Build release binary
        run: cargo build --release --locked

      - name: Setup SSH
        run: |
          mkdir -p ~/.ssh
          echo "${{ secrets.EC2_SSH_KEY }}" > ~/.ssh/deploy_key
          chmod 600 ~/.ssh/deploy_key
          echo "${{ secrets.EC2_KNOWN_HOSTS }}" >> ~/.ssh/known_hosts

      - name: Deploy to EC2
        run: |
          scp -i ~/.ssh/deploy_key -o StrictHostKeyChecking=yes \
            target/release/ptodd \
            ec2-user@54.83.192.65:/tmp/kiss-server-new
          ssh -i ~/.ssh/deploy_key ec2-user@54.83.192.65 \
            "sudo systemctl stop kiss-server && \
             sudo mv /tmp/kiss-server-new /usr/local/bin/kiss-server && \
             sudo chmod +x /usr/local/bin/kiss-server && \
             sudo systemctl start kiss-server"

      - name: Verify service is active
        run: |
          ssh -i ~/.ssh/deploy_key ec2-user@54.83.192.65 \
            "sudo systemctl is-active kiss-server"

      - name: Create GitHub Release
        uses: softprops/action-gh-release@v2
        with:
          tag_name: deploy/${{ github.sha }}
          name: "Deploy ${{ github.sha }}"
          files: target/release/ptodd
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

### GitHub Ruleset for prod Branch (admin-only)
```json
{
  "name": "Protect prod",
  "target": "branch",
  "enforcement": "active",
  "conditions": {
    "ref_name": {
      "include": ["refs/heads/prod"],
      "exclude": []
    }
  },
  "bypass_actors": [
    {
      "actor_type": "RepositoryRole",
      "actor_id": 5,
      "bypass_mode": "always"
    }
  ],
  "rules": [
    { "type": "deletion" },
    { "type": "non_fast_forward" }
  ]
}
```

Note: No `required_status_checks` rule — prod is a deploy target, not a PR merge target. Admins can push directly (bypass_mode: always), matching D-08 (PRs from main → prod via admin).

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `appleboy/scp-action@v0` | `@v1` | 2023 | Updated — use v1 |
| `softprops/action-gh-release@v1` | `@v2` (v2.6.1) | 2024 | v1 deprecated; use v2 |
| Classic branch protection rules API | GitHub Rulesets API | 2023 GA | Project already uses Rulesets (Phase 7) |

**Deprecated/outdated:**
- `appleboy/scp-action` for single-file deploy: functional but introduces path-tree complexity that raw `scp` avoids; not deprecated but not recommended for this use case.

## Open Questions

1. **Does EC2_SSH_KEY secret already exist?**
   - What we know: `gh secret list` returned empty output (no secrets set yet, or output suppressed)
   - What's unclear: Whether the secret was added manually during Phase 9 work
   - Recommendation: Wave 0 plan should include a `gh secret set EC2_SSH_KEY` step with a check-first guard

2. **Does `prod` branch need to exist before cd.yml is merged to main?**
   - What we know: cd.yml triggers only on pushes to `prod`; the workflow file can exist on `main` without effect
   - What's unclear: Whether having a workflow that references a non-existent branch causes any GitHub validation error
   - Recommendation: Create `prod` branch in the same wave or before the first test trigger; it does not block merging cd.yml to main

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|-------------|-----------|---------|----------|
| gh CLI | Secret setup, branch protection script | Yes (local) | 2.87.3 | — |
| SSH/SCP | Deploy steps (on runner) | Yes | OpenSSH 10.2p1 (local); pre-installed on ubuntu-latest | — |
| EC2 instance at 54.83.192.65 | Deploy target | Yes (Phase 8 complete) | Amazon Linux 2023 | — |
| systemd kiss-server unit | Stop/start on EC2 | Yes (Phase 9 complete) | — | — |
| `prod` branch | CD trigger | No — does not exist yet | — | Must create in Wave 0 |
| `EC2_SSH_KEY` GitHub secret | SSH auth in workflow | Unknown — secret list empty | — | Must add in Wave 0 |
| `EC2_KNOWN_HOSTS` GitHub secret | known_hosts setup | No — not set | — | Must add in Wave 0; host key captured: see Pattern 2 |

**Missing dependencies with no fallback:**
- `prod` branch — blocks CD trigger; must be created before testing
- `EC2_SSH_KEY` secret — blocks SSH/SCP in workflow; must be set before first deploy run
- `EC2_KNOWN_HOSTS` secret — blocks StrictHostKeyChecking=yes; must be set before first deploy run

**Missing dependencies with fallback:**
- None

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (unit/integration) |
| Config file | rust-toolchain.toml (toolchain pin) |
| Quick run command | `cargo test --locked` |
| Full suite command | `cargo test --locked` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CD-01 | Push to prod triggers workflow | manual smoke | Merge PR to prod, observe Actions UI | N/A — external |
| CD-02 | `cargo build --release` produces binary | manual smoke | `ls -la target/release/ptodd` after workflow run | N/A — CI artifact |
| CD-03 | Binary replaced atomically on EC2 | manual smoke | `ssh ec2-user@54.83.192.65 "ls -la /usr/local/bin/kiss-server"` after deploy | N/A — infrastructure |
| CD-04 | Service active after deploy | manual smoke | Workflow step exits 0; `systemctl is-active kiss-server` on EC2 | N/A — infrastructure |
| CD-05 | GitHub Release created with binary asset | manual smoke | `gh release view` after workflow run | N/A — GitHub API |

All CD requirements are integration/smoke tests against real infrastructure — not automatable as unit tests. The health check (CD-04) is validated in-workflow by the "Verify service is active" step itself.

### Sampling Rate
- **Per task commit:** `cargo test --locked` (ensures no Rust regression)
- **Per wave merge:** `cargo test --locked` + manual trigger of CD workflow against prod
- **Phase gate:** Full suite green + successful end-to-end deploy run before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] No test infrastructure gaps — existing `cargo test` is sufficient; CD requirements are infrastructure smoke tests

## Sources

### Primary (HIGH confidence)
- GitHub Actions workflow syntax — `on.push.branches`, `permissions`, step structure
- Project `ci.yml` — confirmed toolchain versions (1.93.1), cache settings, checkout@v4
- Project `Cargo.toml` — confirmed `package.name = "ptodd"` → binary is `target/release/ptodd`
- Project `setup-branch-protection.sh` — confirmed ruleset API pattern (POST/PUT, actor_id 5)
- `ssh-keyscan 54.83.192.65` — captured actual EC2 host keys (ecdsa-sha2-nistp256, ssh-ed25519)
- STATE.md blocker notes — confirmed scp-action path issue and systemctl restart vs is-active distinction

### Secondary (MEDIUM confidence)
- WebFetch of softprops/action-gh-release GitHub README — current version v2.6.1 (2026-03-16), `tag_name` parameter behavior confirmed
- WebFetch of appleboy/ssh-action README — v1 current, `key` parameter for private key confirmed
- WebFetch of appleboy/scp-action action.yml — `strip_components` parameter confirmed, path-tree behavior confirmed

### Tertiary (LOW confidence)
- WebSearch results on `ssh-keyscan` known_hosts pattern — standard pattern, multiple sources agree

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — versions verified against GitHub (softprops v2.6.1 confirmed March 2026)
- Architecture: HIGH — binary name from Cargo.toml (direct read), SCP path behavior confirmed from action.yml + issue tracker
- Pitfalls: HIGH — two flagged blockers from STATE.md independently confirmed via source inspection

**Research date:** 2026-03-24
**Valid until:** 2026-04-24 (stable tooling; softprops/action-gh-release version may update but v2 tag is stable)
