# Feature Landscape

**Domain:** CI/CD pipeline, AWS EC2 deployment, DNS routing, GitHub repository configuration
**Researched:** 2026-03-10
**Confidence:** HIGH

## Category 1: GitHub Actions CI Pipeline

### Table Stakes

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| `cargo test` in CI | Every Rust project runs tests in CI | LOW | 83 existing tests |
| `cargo clippy -- -D warnings` | Standard Rust CI lint check | LOW | Pre-commit hook already runs this locally |
| `cargo fmt -- --check` | Format check prevents style drift | LOW | One step in workflow |
| CI runs on push + PR to main | Must block merges on failure | LOW | `on: [push, pull_request]` |
| Cargo registry + target caching | Without cache, full rebuild ~2min per run | LOW | `Swatinem/rust-cache@v2` |

### Differentiators

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| `rust-toolchain.toml` in repo | Prevents surprise lint failures from toolchain updates | LOW | One file, prevents CI churn |
| Separate fmt/clippy/test steps | Granular failure messages | LOW | Single job, multiple steps |

### Anti-Features

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Nightly toolchain | Access to unstable features | Breaks without warning; not needed for stdlib-only project | `stable` channel |
| `cargo audit` in CI | Security vulnerability scanning | External network dep; false positives block PRs | Run manually or on schedule |
| Code coverage reporting | Nice metric | Adds tarpaulin/llvm-cov complexity; no value for a learning project at this stage | Skip for v1.1 |
| `actions-rs/*` actions | Familiar from tutorials | Unmaintained, archived | `dtolnay/rust-toolchain@stable` |

---

## Category 2: GitHub Branch Protection

### Table Stakes

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Require PR before merging to main | Prevents direct pushes; enforces review gate | LOW | Settings → Branches → Add rule |
| Require CI status check to pass | Blocks merging broken code | LOW | Must match **exact** workflow job name |

### Anti-Features

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| "Do not allow bypassing above settings" for admins | Maximum enforcement | Solo dev: locks you out permanently if misconfigured | Leave admin bypass enabled |
| Require code owner review | Team enforcement | No team → blocks all PRs forever | Skip |
| Require linear history | Clean git log | Incompatible with squash-merge milestone strategy | Skip |

---

## Category 3: AWS EC2 Deployment

### Table Stakes

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| kiss-server binary on EC2, running | The core goal | MEDIUM | Build on CI runner, SCP to EC2 |
| systemd unit file | Persists across reboots; auto-restarts on crash | LOW | `Restart=on-failure` |
| Elastic IP attached | Static IP; ephemeral IP breaks DNS on every stop/start | LOW | AWS Console: Allocate → Associate |
| Security Group: port 80 inbound | Public web access | LOW | Port 22 restricted to your IP `/32` |
| iptables redirect 80 → 8080 | Non-root process cannot bind port 80 | LOW | PREROUTING rule, persistent via iptables-save |
| Static files at `/var/www/ptodd.org/` | kiss-server needs `--root` directory | LOW | Must exist before service starts |

**What "running" means operationally:**
- `systemctl is-active kiss-server` returns `active`
- `curl http://localhost:8080/` from the instance returns 200
- `curl http://[elastic-ip]/` from outside returns 200
- Service survives reboot (`systemctl enable kiss-server`)
- Logs visible via `journalctl -u kiss-server`

---

## Category 4: GoDaddy DNS → EC2 Routing

### Table Stakes

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| A record `@` → Elastic IP | `ptodd.org` resolves to EC2 | LOW | GoDaddy DNS manager |
| CNAME `www` → `@` | `www.ptodd.org` works | LOW | Without this, www returns NXDOMAIN |
| TTL 600s during setup | Faster iteration during DNS testing | LOW | Lower before changing; raise to 3600s once stable |

**End-to-end routing:**
```
Browser: ptodd.org
  → GoDaddy DNS: A record @ → [Elastic IP]
  → AWS Security Group: port 80 allowed
  → EC2 iptables: 80 → 8080
  → kiss-server: serves /var/www/ptodd.org/index.html
  → 200 OK, Hello World HTML
```

---

## Category 5: CD Pipeline (prod branch → deploy)

### Table Stakes

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Trigger on push to `prod` branch | Automation signal | LOW | `on: push: branches: [prod]` |
| Build `--release` binary on CI runner | Optimized binary for production | LOW | `cargo build --release` |
| SCP binary to EC2 | Delivery mechanism | LOW | `appleboy/scp-action@v1` |
| SSH: stop → mv → start service | Atomic safe replacement | LOW | See PITFALLS: stop before mv, never in-place SCP |
| Health check post-deploy | Verify deploy actually worked | LOW | `systemctl is-active kiss-server` — not optional |
| GitHub Release created | Versioned artifact | MEDIUM | `softprops/action-gh-release@v2` |
| Binary attached to GitHub Release | Downloadable artifact | LOW | Release asset upload |

### Anti-Features

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Zero-downtime deploy | Professional | Overkill for static site with ~0 traffic; requires process supervision complexity | Simple stop/start is fine |
| Blue/green deployment | Maximum reliability | Requires 2x infrastructure | Future v2+ consideration |
| Auto-deploy on push to main | Faster iteration | Bypasses the deliberate prod promotion gate | Keep prod branch separation |

---

## Category 6: Build Status Badge

### Table Stakes

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| CI badge in README.md | Industry standard for open source repos | LOW | One line of Markdown |

**How it works:** GitHub generates an SVG at:
```
https://github.com/{owner}/{repo}/actions/workflows/{workflow}.yml/badge.svg
```
The SVG reflects the latest workflow run on the default branch. Add to README:
```markdown
![CI](https://github.com/ptdecker/kiss-server/actions/workflows/ci.yml/badge.svg)
```

---

## Category 7: Documentation

### Table Stakes

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| `docs/ci-cd.md` | Future-you needs to understand the pipeline | LOW | Markdown file in repo |
| `README.md` update | Repo front page should reflect current state | LOW | Badge + description + usage + deployment |

**docs/ci-cd.md should cover:**
- CI workflow: what it checks, how to read results, how to fix failures
- CD workflow: how to deploy (push to prod), what happens automatically
- Setup from scratch: GitHub Secrets, branch protection, EC2 prerequisites
- EC2 maintenance: SSH access, log viewing (`journalctl`), manual restart

---

## Feature Dependencies

```
GitHub branch protection
    └──requires──> CI workflow (check must have run at least once before rule can reference it)

CD pipeline
    └──requires──> EC2 running (SSH target must exist)
    └──requires──> GitHub Secrets set (EC2_HOST, EC2_SSH_KEY, EC2_USER)
    └──produces──> GitHub Release

DNS routing
    └──requires──> Elastic IP allocated (A record target must be stable)
    └──requires──> EC2 responding on port 80

Hello World static site
    └──requires──> EC2 running with kiss-server
    └──requires──> /var/www/ptodd.org/index.html on EC2

Build badge
    └──requires──> CI workflow file exists and has run at least once

docs/ci-cd.md + README badge
    └──requires──> CI workflow file exists (for badge URL)
```

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| GitHub Actions CI | HIGH | LOW | P1 |
| Branch protection | HIGH | LOW | P1 |
| EC2 + systemd | HIGH | MEDIUM | P1 |
| Elastic IP + Security Group | HIGH | LOW | P1 |
| iptables redirect | HIGH | LOW | P1 |
| Hello World static site | HIGH | LOW | P1 |
| GoDaddy DNS routing | HIGH | LOW | P1 |
| CD pipeline (deploy) | HIGH | MEDIUM | P1 |
| GitHub Release creation | MEDIUM | LOW | P2 |
| Build badge | LOW | LOW | P2 |
| docs/ci-cd.md | MEDIUM | LOW | P2 |
| README update | MEDIUM | LOW | P2 |

## Sources

- GitHub Actions official documentation — workflow syntax, triggers, permissions
- AWS EC2 user guide — instance types, security groups, Elastic IP
- GoDaddy DNS help center — A record, CNAME, TTL
- systemd service documentation — unit file format

---
*Feature research for: CI/CD ops & deployment, AWS EC2, GoDaddy DNS (v1.1)*
*Researched: 2026-03-10*
