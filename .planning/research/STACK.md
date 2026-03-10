# Stack Research

**Domain:** CI/CD pipeline, AWS EC2 deployment, DNS routing for Rust binary
**Researched:** 2026-03-10
**Confidence:** HIGH — GitHub Actions official docs, AWS official docs, GoDaddy help pages

## Recommended Stack

### GitHub Actions CI

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| `ubuntu-latest` runner | — | CI build environment | x86_64 Linux; matches t3.micro EC2 arch; no cross-compilation needed |
| `actions/checkout` | `@v4` | Checkout repo | Current stable major; widest adoption |
| `dtolnay/rust-toolchain` | `@stable` | Rust toolchain setup | Minimal, purpose-built; maintained; replaces unmaintained `actions-rs/*` |
| `Swatinem/rust-cache` | `@v2` | Cargo cache | v2.7.x actively maintained; standard Rust ecosystem choice; handles key construction and cleanup automatically |

### GitHub Actions CD

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| `appleboy/scp-action` | `@v1` | SCP binary to EC2 | v1.0.0 (April 2025); 36k+ repos; straightforward SSH-based copy |
| `appleboy/ssh-action` | `@v1` | SSH to restart service | v1.2.4 (2026); 130k+ repos; executes `systemctl restart` |
| `softprops/action-gh-release` | `@v2` | Create GitHub Release | Standard release action; supports asset upload; `contents: write` permission required |

SSH/SCP deploy is correct over AWS CodeDeploy — CodeDeploy requires IAM agent on EC2, appspec.yml, and deployment groups. Two YAML blocks vs an entire AWS service for one instance.

### AWS Infrastructure

| Technology | Version/Tier | Purpose | Why Recommended |
|------------|-------------|---------|-----------------|
| EC2 `t3.micro` | x86_64, ~$8.47/mo | Host kiss-server | x86_64 matches GitHub Actions `ubuntu-latest` runner — no cross-compilation; free tier eligible |
| Amazon Linux 2023 AMI | Current | EC2 OS | AWS-native, systemd included, supported through 2029. **Do not use Amazon Linux 2 — EOL June 30, 2026.** |
| Elastic IP | Free when attached | Static public IP | EC2 public IPs are ephemeral; Elastic IP is free when attached to a running instance |
| Security Group | — | Firewall | Port 80/TCP open to `0.0.0.0/0`; port 22/TCP restricted to deployer IP `/32` |
| systemd | Included in AL2023 | Process management | Standard Linux service manager; `Type=simple`, `Restart=on-failure` |

### DNS

| Technology | Purpose | Why Recommended |
|------------|---------|-----------------|
| GoDaddy A record (`@`) | Apex domain → Elastic IP | A record required for zone apex per DNS spec; GoDaddy rejects CNAME on `@` |
| GoDaddy CNAME (`www`) | `www.ptodd.org` → `ptodd.org` | Covers both bare domain and www subdomain |

Route 53 is **not needed** — adds $0.50/mo hosted zone + nameserver delegation with no benefit for a single static IP.

## GitHub Secrets Required

| Secret | Value | Used By |
|--------|-------|---------|
| `EC2_SSH_KEY` | ED25519 private key PEM (Unix line endings) | CD deploy via SCP + SSH |
| `EC2_HOST` | Elastic IP address | CD deploy |
| `EC2_USER` | `ec2-user` (Amazon Linux default) | CD deploy |

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| `actions-rs/*` (actions-rs/toolchain, etc.) | Unmaintained, archived | `dtolnay/rust-toolchain@stable` |
| `actions/upload-artifact@v3` | Deprecated January 30, 2025; no longer works | `actions/upload-artifact@v4` |
| EC2 `t4g.micro` (Graviton/arm64) | arm64 binary fails on x86_64 runner output with `Exec format error` | `t3.micro` (x86_64) |
| Amazon Linux 2 | EOL June 30, 2026 | Amazon Linux 2023 |
| AWS CodeDeploy | Heavy: IAM agent on EC2, appspec.yml, deployment groups — overkill for 1 instance | `appleboy/scp-action` + `appleboy/ssh-action` |
| AWS Route 53 | $0.50/mo hosted zone; unnecessary for single static IP | GoDaddy A record directly to Elastic IP |
| CNAME at apex `@` | Prohibited by DNS spec (RFC 1912); GoDaddy rejects it | A record for `@`, CNAME for `www` |

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| SSH/SCP deploy | AWS CodeDeploy | If managing multiple instances or need rollback automation |
| SSH/SCP deploy | AWS Systems Manager (SSM) | If you want no open port 22; adds IAM complexity |
| Elastic IP | Route 53 + dynamic IP | Never for a static single instance |
| Amazon Linux 2023 | Ubuntu 24.04 LTS | Either works; AL2023 has better AWS integration |
| t3.micro | t4g.micro (Graviton) | Only if you add cross-compilation to CI — saves ~$5.50/mo |

## EC2 systemd Unit File

```ini
[Unit]
Description=kiss-server HTTP server
After=network.target

[Service]
Type=simple
User=ec2-user
ExecStart=/usr/local/bin/kiss-server --root /var/www/ptodd.org --port 8080
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

Port 8080 (not 80) — `ec2-user` (non-root) cannot bind ports below 1024. Use iptables PREROUTING redirect: `80 → 8080`. Port 8080 does NOT need to be open in the Security Group.

## Sources

- [dtolnay/rust-toolchain](https://github.com/dtolnay/rust-toolchain) — replacement for unmaintained actions-rs
- [Swatinem/rust-cache](https://github.com/Swatinem/rust-cache) — caching strategy documentation
- [appleboy/scp-action](https://github.com/appleboy/scp-action) — SCP deploy action docs
- [appleboy/ssh-action](https://github.com/appleboy/ssh-action) — SSH action docs
- [softprops/action-gh-release](https://github.com/softprops/action-gh-release) — release action
- AWS EC2 instance types — t3.micro pricing and architecture
- Amazon Linux 2023 release notes — EOL dates
- GoDaddy DNS help — A record vs CNAME at apex

---
*Stack research for: CI/CD ops & deployment, AWS EC2, GoDaddy DNS (v1.1)*
*Researched: 2026-03-10*
