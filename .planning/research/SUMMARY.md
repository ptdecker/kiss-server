# Project Research Summary

**Project:** ptodd — kiss-server CI/CD ops & deployment
**Domain:** GitHub Actions CI/CD, AWS EC2, GoDaddy DNS, GitHub Releases
**Researched:** 2026-03-10
**Confidence:** HIGH

## Executive Summary

The v1.1 milestone is a classic "get it deployed" ops problem for a small Rust binary. The recommended approach is deliberate and minimal: GitHub Actions for CI (fmt + clippy + test) gated by branch protection, then a manual `prod` branch promotion that triggers a CD workflow to SCP the binary to a single EC2 `t3.micro` instance running Amazon Linux 2023. No managed services, no container orchestration, no reverse proxy — just a compiled binary under systemd with an iptables redirect from port 80 to 8080. GoDaddy DNS routes directly to an Elastic IP via an A record; Route 53 is unnecessary overhead for a single static IP.

The single architectural decision that prevents the most pain is matching CPU architecture end-to-end: GitHub Actions `ubuntu-latest` runners build x86_64, so EC2 must be `t3.micro` (x86_64), not the cheaper `t4g` (arm64). This avoids the `Exec format error` that causes silent green-CI / broken-prod failures. The second highest-value safeguard is the post-deploy health check (`systemctl is-active kiss-server || exit 1`) because `systemctl restart` exits 0 even when the service crashes immediately — without this check, the pipeline can report success while production is down.

Key risks are all avoidable with known patterns: CRLF in SSH private keys breaks OpenSSH (validate with `ssh-keygen -y -f` before storing as a secret), in-place SCP over a running binary risks SIGBUS (always SCP to `/tmp/` then `mv` atomically), and branch protection must be configured only after CI has run at least once (so the job name appears in the autocomplete dropdown). All of these are one-time setup risks, not ongoing operational complexity.

## Key Findings

### Recommended Stack

GitHub Actions is the only CI/CD layer needed. Use `dtolnay/rust-toolchain@stable` (not the archived `actions-rs/*`), `Swatinem/rust-cache@v2` for Cargo caching, `appleboy/scp-action@v1` + `appleboy/ssh-action@v1` for SSH/SCP deploy, and `softprops/action-gh-release@v2` for GitHub Release creation. AWS CodeDeploy and Route 53 are explicitly not recommended — they add IAM agents, appspec files, and hosted zone fees for zero benefit on a single instance.

**Core technologies:**
- `dtolnay/rust-toolchain@stable`: Rust toolchain setup — replaces unmaintained `actions-rs/*`; purpose-built, actively maintained
- `Swatinem/rust-cache@v2`: Cargo cache — handles key construction and cleanup; pair with `CARGO_INCREMENTAL=0` to prevent cache bloat
- `appleboy/scp-action@v1` + `appleboy/ssh-action@v1`: SSH/SCP deploy — 130k+ repo adoption; straightforward; no IAM agent required
- `softprops/action-gh-release@v2`: GitHub Releases — requires `permissions: contents: write` on the CD job
- EC2 `t3.micro` (x86_64, Amazon Linux 2023): Compute — x86_64 matches CI runner; AL2023 supported through 2029; Amazon Linux 2 EOL June 30, 2026
- Elastic IP: Static IP — EC2 public IPs are ephemeral; Elastic IP is free when attached; everything downstream (DNS, SSH host keys) depends on it being stable
- GoDaddy A record (`@`) + CNAME (`www`): DNS — A record required at zone apex per RFC 1912; GoDaddy correctly rejects CNAME at `@`

### Expected Features

**Must have (table stakes):**
- GitHub Actions CI (fmt + clippy + test) — blocks merges on broken code
- Branch protection: require PR + CI pass before merging to main
- EC2 `t3.micro` with Amazon Linux 2023, systemd unit, Elastic IP, Security Group
- iptables PREROUTING redirect (80 → 8080) — non-root process cannot bind port 80
- Static site at `/var/www/ptodd.org/` with Hello World `index.html`
- GoDaddy A record (`@` → Elastic IP) + CNAME (`www` → `@`)
- CD pipeline: push to `prod` → build `--release` → SCP to `/tmp/` → atomic replace → health check
- `rust-toolchain.toml` in repo root — prevents surprise lint failures from toolchain drift

**Should have (differentiators/polish):**
- GitHub Release created on each CD deploy with binary attached
- Build status badge in README.md
- `docs/ci-cd.md` — documents the pipeline for future-you

**Defer (v2+):**
- Zero-downtime or blue/green deployment — overkill for static site with minimal traffic
- `cargo audit` in CI — external network dependency; false positives block PRs
- Code coverage reporting — tarpaulin/llvm-cov complexity with no learning value at this stage
- Auto-deploy on push to main — bypasses the deliberate prod promotion gate

### Architecture Approach

The system has three horizontal layers: GitHub (source + CI/CD), AWS (compute + networking), and GoDaddy (DNS). The CI workflow (`ci.yml`) runs on push/PR to main and posts a status check that branch protection requires before merging. The CD workflow (`cd.yml`) triggers on push to the `prod` branch — which is only updated manually from `main` — builds the release binary, SCPs it to `/tmp/` on EC2, stops the systemd service, atomically replaces the binary via `mv`, restarts the service, and verifies it is active. A GitHub Release is then created with the binary attached as an asset.

**Major components:**
1. `.github/workflows/ci.yml` — fmt + clippy + test; posts status check named `ci` (this exact name is referenced by branch protection)
2. `.github/workflows/cd.yml` — release build, SCP to `/tmp/`, atomic replace on EC2, `systemctl is-active` health check, GitHub Release
3. EC2 `t3.micro` (Amazon Linux 2023) — runs kiss-server as a systemd service on port 8080; iptables PREROUTING redirects port 80 inbound
4. Elastic IP — stable public IP; anchors DNS and SSH host key
5. GoDaddy DNS — A record `@` → Elastic IP; CNAME `www` → `@`
6. GitHub Secrets — `EC2_HOST`, `EC2_SSH_KEY`, `EC2_USER` — bridge between GitHub Actions and EC2

### Critical Pitfalls

1. **Architecture mismatch (CI x86_64 vs EC2 arm64)** — use `t3.micro` (x86_64); the `Exec format error` surfaces only at `systemctl start`, not during SCP, so CI can show green while prod is broken
2. **Branch protection lockout from check name mismatch** — run CI at least once before configuring branch protection; use the autocomplete dropdown to select the job name; never enable "do not allow bypassing" for a solo developer
3. **SSH private key CRLF rejection** — strip CRLF with `tr -d '\r'` before storing as GitHub Secret; validate with `ssh-keygen -y -f key.pem` before committing to a secret
4. **`systemctl restart` exits 0 on immediate crash (silent deploy failure)** — always follow with `systemctl is-active kiss-server || (journalctl -u kiss-server -n 30 && exit 1)`; this is the single highest-value safeguard in the entire pipeline
5. **In-place SCP over running binary causes SIGBUS** — always SCP to `/tmp/`, then `stop → mv → chmod → start`; `mv` is atomic on Linux; SCP is not

## Implications for Roadmap

The dependency chain documented in ARCHITECTURE.md constrains phase ordering: CI must run before branch protection can reference its check name; Elastic IP must exist before DNS is set; EC2 must be responding before CD can be validated. The following 7-phase structure respects those hard dependencies and groups related work.

### Phase 1: CI Workflow
**Rationale:** No external dependencies. Enables everything else. Branch protection cannot be configured until CI has run at least once to register the `ci` check name in GitHub's system.
**Delivers:** `.github/workflows/ci.yml` (fmt + clippy + test), `rust-toolchain.toml`, first green CI run on main
**Addresses:** Cargo CI table stakes, toolchain pinning differentiator
**Avoids:** Toolchain version drift causing surprise lint failures (Pitfall 13); cache bloat from unbounded `target/` growth (Pitfall 7)

### Phase 2: Branch Protection
**Rationale:** Must come after first CI run so the job name `ci` appears in the autocomplete dropdown. Logical quality gate before any work proceeds via PR.
**Delivers:** Branch protection rule on main requiring PR + `ci` check to pass
**Avoids:** Check name mismatch permanently blocking all PRs (Pitfall 2); admin bypass disabled for solo dev leaving no recovery path (Pitfall 2)

### Phase 3: AWS Infrastructure
**Rationale:** Elastic IP must exist before DNS configuration. Security Group must be correct before any connectivity testing. The x86_64 architecture decision must be locked before writing CD workflow YAML.
**Delivers:** EC2 `t3.micro` (Amazon Linux 2023), Elastic IP allocated and attached, Security Group (port 80 open to world, port 22 restricted to deployer IP)
**Avoids:** Architecture mismatch producing `Exec format error` at deploy time (Pitfall 1); ephemeral EC2 IP invalidating DNS on every stop/start (Pitfall 11); SSH port 22 open to automated scanners (Pitfall 4)

### Phase 4: EC2 Service Setup
**Rationale:** Service must be verified manually before CD automates it. Writing the systemd unit file and iptables rule first and testing by hand prevents "automated deployment of a misconfigured service" — the unit file must include `--root /var/www/ptodd.org --port 8080`.
**Delivers:** systemd `kiss-server.service` unit, iptables PREROUTING rule (80 → 8080), `/var/www/ptodd.org/index.html`, manual verification that `curl http://[elastic-ip]/` returns 200
**Avoids:** Missing `--root` causing immediate crash and systemd rate-limiting (Pitfall 12); wrong port in Security Group vs iptables (Pitfall 4); running kiss-server as root to bind port 80 (Architecture anti-pattern)

### Phase 5: DNS Configuration
**Rationale:** EC2 must be responding on port 80 before DNS is configured — otherwise DNS propagation delays are indistinguishable from service failures. Lower TTL to 300s before making changes.
**Delivers:** GoDaddy A record `@` → Elastic IP; CNAME `www` → `@`; both `ptodd.org` and `www.ptodd.org` return 200
**Avoids:** CNAME at zone apex rejected by GoDaddy (Pitfall 9); missing `www` CNAME causing NXDOMAIN (Pitfall 14); default TTL delaying debugging by hours (Pitfall 9)

### Phase 6: CD Pipeline
**Rationale:** Requires EC2 running (SSH target), GitHub Secrets set, and CI pipeline established. This is the most risk-dense phase — CRLF key issue, in-place SCP, silent restart failure, and GitHub Release 403 all live here.
**Delivers:** `.github/workflows/cd.yml` triggered by push to `prod` branch; atomic binary replacement (`/tmp/` → `mv`); `systemctl is-active` health check; GitHub Release with binary asset; first successful end-to-end deploy via `git push origin main:prod`
**Avoids:** CRLF SSH key rejection (Pitfall 3); in-place SCP SIGBUS (Pitfall 5); silent deploy failure from `systemctl restart` exits 0 (Pitfall 6); GitHub Release HTTP 403 from missing `permissions: contents: write` (Pitfall 10)

### Phase 7: Badge, Docs, README
**Rationale:** CI workflow file must exist for the badge URL to resolve. This is finishing work — low complexity, purely additive, no production dependency.
**Delivers:** Build status badge in README.md; `docs/ci-cd.md` covering pipeline operation, how to deploy, EC2 maintenance (journalctl, manual restart), and setup from scratch; README description updated to reflect current state
**Avoids:** Nothing safety-critical — this is polish that pays forward for the next developer (or future-you)

### Phase Ordering Rationale

- CI before branch protection: the check name must appear in GitHub's registry before it can be selected as a required check
- Elastic IP before DNS: Elastic IP is the A record target; must be stable before anything references it
- EC2 service setup before DNS: EC2 must respond on port 80 so DNS issues can be distinguished from service issues
- EC2 service setup before CD: CD automates what must first work manually; automating a broken config multiplies the failure
- Full CI + live EC2 before CD: CD needs the cache infrastructure and a live SSH target simultaneously
- Badge and docs last: purely cosmetic; nothing blocks or is blocked by them

### Research Flags

Phases with standard, well-documented patterns (skip `/gsd:research-phase`):
- **Phase 1 (CI Workflow):** GitHub Actions Rust CI is extensively documented; full workflow YAML is provided in ARCHITECTURE.md
- **Phase 2 (Branch Protection):** GitHub UI configuration with no code changes
- **Phase 3 (AWS Infrastructure):** Standard EC2 + Elastic IP + Security Group setup; no novel patterns
- **Phase 5 (DNS Configuration):** Straightforward GoDaddy A record + CNAME; record values known after Phase 3
- **Phase 7 (Badge, Docs, README):** One-line Markdown badge; standard documentation task

Phases that may benefit from targeted research during planning:
- **Phase 4 (EC2 Service Setup):** Amazon Linux 2023 iptables persistence — confirm the exact `dnf install iptables-services` + `service iptables save` command sequence on AL2023 specifically before executing
- **Phase 6 (CD Pipeline):** `appleboy/scp-action@v1` destination path behavior — the action appends the source directory structure to the target, so `/tmp/` becomes `/tmp/target/release/kiss-server`; validate this against the actual action output before finalizing the `mv` path in the SSH step

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | GitHub Actions, AWS, and GoDaddy official docs; all action versions verified against current releases as of 2026-03-10 |
| Features | HIGH | Dependency chain is complete; every feature has explicit inclusion/exclusion rationale; anti-features documented with alternatives |
| Architecture | HIGH | Full workflow YAML provided; systemd unit file specified; all three data flows (CI, CD, live traffic) documented end-to-end |
| Pitfalls | HIGH | Each pitfall includes detection pattern, prevention steps, and phase assignment; the cross-phase "green CI, broken prod" failure mode explicitly identified |

**Overall confidence:** HIGH

### Gaps to Address

- **iptables persistence on Amazon Linux 2023:** The exact persistence command differs between AL2 and AL2023. Research confirms `dnf install iptables-services` + `service iptables save` but this should be verified on the actual instance before the Phase 4 execution step.
- **`appleboy/scp-action@v1` destination path structure:** The action appends the source path structure to the target directory. The SSH step's `mv` command must use the resolved path (e.g., `/tmp/target/release/kiss-server`, not `/tmp/kiss-server`). Validate on first deploy before writing the final `mv` command.
- **GitHub Actions IP ranges for Security Group rule:** If SSH (port 22) is restricted to your IP `/32`, the CD pipeline cannot SSH from GitHub Actions runners. Options: (a) add a broader SSH rule accepting any source IP (key auth is still required; ED25519 brute force is infeasible), or (b) use AWS Systems Manager Session Manager instead of SSH (adds IAM complexity). This is a security policy decision to make during Phase 3 planning.

## Sources

### Primary (HIGH confidence)
- [dtolnay/rust-toolchain](https://github.com/dtolnay/rust-toolchain) — Rust toolchain action, replacement for `actions-rs/*`
- [Swatinem/rust-cache](https://github.com/Swatinem/rust-cache) — caching strategy, `CARGO_INCREMENTAL=0` rationale
- [appleboy/scp-action](https://github.com/appleboy/scp-action) — SCP deploy action, version history, path behavior
- [appleboy/ssh-action](https://github.com/appleboy/ssh-action) — SSH action, version history, 130k+ repo adoption
- [softprops/action-gh-release](https://github.com/softprops/action-gh-release) — release action, `permissions: contents: write` requirement
- GitHub Actions official documentation — workflow syntax, branch protection, status checks, permissions model
- AWS EC2 user guide — instance types (t3.micro vs t4g architecture), Elastic IP allocation and association, Security Groups
- Amazon Linux 2023 release notes — EOL dates (AL2 EOL June 30, 2026; AL2023 supported through 2029)
- GoDaddy DNS help center — A record vs CNAME at zone apex, TTL behavior

### Secondary (MEDIUM confidence)
- Community patterns for atomic binary replacement via `mv` on Linux
- systemd documentation — `Restart=on-failure` vs `Restart=always` behavior on intentional stop during deployment

---
*Research completed: 2026-03-10*
*Ready for roadmap: yes*
