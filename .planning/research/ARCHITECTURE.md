# Architecture Research

**Domain:** CI/CD pipeline, AWS EC2 deployment, DNS routing for Rust binary
**Researched:** 2026-03-10
**Confidence:** HIGH

## System Overview

```
┌─────────────────────────────────────────────────────────────┐
│                GitHub (ptdecker/kiss-server)                 │
│                                                              │
│  branches: main (protected), prod (CD trigger), gsd/*       │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │              GitHub Actions                           │   │
│  │  ci.yml  — runs on push/PR to main                   │   │
│  │            cargo fmt + clippy + test                  │   │
│  │            posts status check (required for merge)    │   │
│  │                                                       │   │
│  │  cd.yml  — runs on push to prod                      │   │
│  │            cargo build --release                      │   │
│  │            SCP binary → EC2                          │   │
│  │            SSH: stop → mv → start                    │   │
│  │            systemctl is-active (health check)        │   │
│  │            Create GitHub Release + attach binary      │   │
│  └─────────────────────────┬────────────────────────────┘   │
│                            │ SSH/SCP (GitHub Secrets)        │
│  Releases tab:             │                                  │
│  prod-{sha} tag +          │                                  │
│  kiss-server binary        │                                  │
└────────────────────────────┼────────────────────────────────┘
                             │
                             ▼
┌─────────────────────────────────────────────────────────────┐
│                         AWS                                  │
│                                                              │
│  Elastic IP (static) ◄──── Security Group                   │
│       │                    • port 80/TCP: 0.0.0.0/0         │
│       │                    • port 22/TCP: your-ip/32         │
│       │                                                      │
│       ▼                                                      │
│  EC2 t3.micro (Amazon Linux 2023, x86_64)                   │
│  ├── iptables PREROUTING: port 80 → 8080                    │
│  ├── /usr/local/bin/kiss-server (deployed binary)           │
│  ├── /var/www/ptodd.org/ (static site root)                 │
│  └── systemd: kiss-server.service                           │
│       User=ec2-user                                          │
│       ExecStart=kiss-server --root /var/www/ptodd.org        │
│                             --port 8080                      │
│       Restart=on-failure                                     │
└────────────────────────────┬────────────────────────────────┘
                             │ Elastic IP
                             ▼
┌─────────────────────────────────────────────────────────────┐
│                       GoDaddy DNS                            │
│  @ A record     → [Elastic IP]                              │
│  www CNAME      → @                                         │
│  TTL: 600s (setup) → 3600s (stable)                        │
└────────────────────────────┬────────────────────────────────┘
                             │ ptodd.org resolves to Elastic IP
                             ▼
                      Browser → ptodd.org
```

## New Components

| Component | Type | Location |
|-----------|------|----------|
| `.github/workflows/ci.yml` | NEW file | GitHub Actions CI workflow |
| `.github/workflows/cd.yml` | NEW file | GitHub Actions CD workflow |
| `rust-toolchain.toml` | NEW file | Pins Rust stable toolchain |
| `docs/ci-cd.md` | NEW file | CI/CD documentation |
| `README.md` | MODIFIED | Add badge, description, deployment notes |
| `/etc/systemd/system/kiss-server.service` | NEW (on EC2) | systemd unit file |
| `/usr/local/bin/kiss-server` | NEW (on EC2) | Deployed binary |
| `/var/www/ptodd.org/index.html` | NEW (on EC2) | Hello World static site |
| GitHub branch protection rule | NEW (repo setting) | Require PR + CI pass for main |
| GitHub Secrets | NEW (repo setting) | EC2_HOST, EC2_SSH_KEY, EC2_USER |
| AWS EC2 instance | NEW (AWS) | t3.micro, Amazon Linux 2023 |
| AWS Elastic IP | NEW (AWS) | Static IP attached to EC2 |
| AWS Security Group | NEW (AWS) | Ports 80 + 22 inbound |
| GoDaddy A record `@` | NEW (DNS) | Elastic IP |
| GoDaddy CNAME `www` | NEW (DNS) | → `@` |

## Architectural Patterns

### Pattern 1: Port Redirect (80 → 8080 via iptables)

**What:** kiss-server listens on port 8080. Kernel-level iptables PREROUTING rule redirects port 80 → 8080 before the packet reaches the process.

**Why:** `ec2-user` cannot bind ports below 1024 without root. Alternatives rejected:
- `CAP_NET_BIND_SERVICE`: must be re-applied after every binary replacement (CD pipeline complexity)
- Run as root: security risk; not worth it for a file server
- Reverse proxy (nginx): unnecessary complexity for a single binary on one instance

**Setup:**
```bash
sudo iptables -t nat -A PREROUTING -p tcp --dport 80 -j REDIRECT --to-port 8080
# Persist across reboots:
sudo iptables-save | sudo tee /etc/iptables/rules.v4
# Or on Amazon Linux 2023:
sudo dnf install iptables-services
sudo service iptables save
```

Port 8080 does **not** need to be open in the Security Group — the redirect happens inside the instance after Security Group allows port 80.

### Pattern 2: Atomic Binary Replacement (stop → mv → start)

**What:** CD pipeline stops the service, SCPs to a temp path, `mv`s into place (atomic on Linux), then starts the service.

**Why:** SCP may truncate and rewrite the target file in-place, causing SIGBUS in the running process. `mv` on the same filesystem is atomic — the directory entry swaps instantly.

**CD deploy sequence:**
```bash
# Step 1: Copy to temp (appleboy/scp-action)
scp target/release/kiss-server ec2-user@$HOST:/tmp/kiss-server-new

# Step 2: Atomic replace (appleboy/ssh-action)
sudo systemctl stop kiss-server
sudo mv /tmp/kiss-server-new /usr/local/bin/kiss-server
sudo chmod +x /usr/local/bin/kiss-server
sudo systemctl start kiss-server

# Step 3: Health check (mandatory — systemctl restart exits 0 even on crash)
systemctl is-active kiss-server || (journalctl -u kiss-server -n 30; exit 1)
```

### Pattern 3: prod Branch as Deployment Gate

**What:** `main` accumulates milestone commits. `prod` branch is explicitly updated from `main` when ready to deploy. Push to `prod` triggers CD.

**Why:** Separates "code merged to main" from "code deployed to production". Gives explicit control over when deployments happen.

**Deployment flow:**
```bash
# When ready to deploy:
git checkout prod
git merge main
git push origin prod
# → triggers cd.yml → deploys to EC2 → creates GitHub Release
```

## CI Workflow Structure

```yaml
# .github/workflows/ci.yml
name: CI
on:
  push:
    branches: [main, 'gsd/**']
  pull_request:
    branches: [main]

env:
  CARGO_INCREMENTAL: "0"    # Disable incremental compilation; saves cache space

jobs:
  ci:                        # ← exact string referenced in branch protection
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - name: Format check
        run: cargo fmt -- --check
      - name: Lint
        run: cargo clippy -- -D warnings
      - name: Test
        run: cargo test
```

`CARGO_INCREMENTAL=0` prevents incremental compilation artifacts from bloating the cache across runners (they're non-reusable and grow unboundedly).

## CD Workflow Structure

```yaml
# .github/workflows/cd.yml
name: CD
on:
  push:
    branches: [prod]

env:
  CARGO_INCREMENTAL: "0"

jobs:
  deploy:
    runs-on: ubuntu-latest
    permissions:
      contents: write          # Required for GitHub Release asset upload
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - uses: Swatinem/rust-cache@v2
      - name: Build release binary
        run: cargo build --release
      - name: Copy binary to EC2
        uses: appleboy/scp-action@v1
        with:
          host: ${{ secrets.EC2_HOST }}
          username: ${{ secrets.EC2_USER }}
          key: ${{ secrets.EC2_SSH_KEY }}
          source: "target/release/kiss-server"
          target: "/tmp/"
      - name: Deploy and restart service
        uses: appleboy/ssh-action@v1
        with:
          host: ${{ secrets.EC2_HOST }}
          username: ${{ secrets.EC2_USER }}
          key: ${{ secrets.EC2_SSH_KEY }}
          script: |
            sudo systemctl stop kiss-server
            sudo mv /tmp/target/release/kiss-server /usr/local/bin/kiss-server
            sudo chmod +x /usr/local/bin/kiss-server
            sudo systemctl start kiss-server
            systemctl is-active kiss-server || (journalctl -u kiss-server -n 30 && exit 1)
      - name: Create GitHub Release
        uses: softprops/action-gh-release@v2
        with:
          tag_name: "prod-${{ github.sha }}"
          name: "Deployment ${{ github.sha }}"
          files: target/release/kiss-server
```

## systemd Unit File

```ini
# /etc/systemd/system/kiss-server.service
[Unit]
Description=kiss-server HTTP/1.1 static file server
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

`Restart=on-failure` (not `always`) — `always` causes restart loops on intentional stop during deployment.

## Data Flows

### CI Flow

```
PR opened against main
  → GitHub triggers ci.yml
  → ubuntu-latest runner
  → checkout → toolchain → cache restore
  → cargo fmt -- --check      (fails fast on formatting)
  → cargo clippy -- -D warnings  (fails on lint)
  → cargo test                 (83 tests)
  → cache save
  → status check posted to PR
  → branch protection: blocks merge if any step failed
```

### CD Flow

```
git push origin main:prod
  → GitHub triggers cd.yml
  → ubuntu-latest runner (x86_64 — matches EC2 arch)
  → checkout → toolchain → cache restore
  → cargo build --release
  → SCP: target/release/kiss-server → EC2:/tmp/
  → SSH: stop → mv → chmod → start → is-active check
  → GitHub Release: tag prod-{sha} + binary asset
  → ptodd.org now serves new binary
```

### Live Traffic Flow

```
Browser: GET http://ptodd.org/
  → GoDaddy DNS: ptodd.org A → [Elastic IP]
  → AWS Security Group: port 80 allowed
  → EC2 iptables PREROUTING: 80 → 8080
  → kiss-server: serve /var/www/ptodd.org/index.html
  → 200 OK, text/html
```

## Phase Build Order

The dependency chain determines phase order:

1. **CI workflow** — no external dependencies; enables everything else
2. **Branch protection** — requires CI to have run at least once (check name must exist)
3. **AWS infrastructure** — EC2, Elastic IP, Security Group (Elastic IP must exist before DNS)
4. **EC2 service setup** — systemd unit, iptables, static site directory, Hello World content
5. **DNS configuration** — GoDaddy A record → Elastic IP (EC2 must be responding)
6. **CD pipeline** — requires EC2 running + GitHub Secrets + CI pipeline complete
7. **Badge + docs + README** — requires CI workflow file to exist (badge URL)

## Integration Points

| Integration | Pattern | Notes |
|-------------|---------|-------|
| GitHub Actions → EC2 | SSH/SCP via GitHub Secrets | Secrets: EC2_HOST, EC2_SSH_KEY, EC2_USER |
| GitHub Actions → GitHub Releases | `softprops/action-gh-release@v2` | Requires `permissions: contents: write` |
| GoDaddy DNS → AWS | A record to Elastic IP | No Route 53 needed |
| EC2 port 80 → 8080 | iptables PREROUTING | Inside instance; Security Group only opens 80 |

## Anti-Patterns

### Anti-Pattern 1: Running kiss-server as root to bind port 80

**What people do:** `sudo ExecStart=...` or `User=root` to bind port 80 directly.
**Why it's wrong:** Unnecessary privilege; vulnerability in kiss-server gives attacker root shell.
**Do this instead:** Run as `ec2-user`, use iptables PREROUTING redirect.

### Anti-Pattern 2: Choosing EC2 arm64 (t4g) with x86_64 CI runner

**What people do:** Pick the cheapest EC2 (`t4g.nano` at ~$3/mo) without checking architecture.
**Why it's wrong:** GitHub Actions `ubuntu-latest` builds x86_64. Binary SCP succeeds silently. Service fails with `Exec format error` at start.
**Do this instead:** Use `t3.micro` (x86_64) for this milestone. No cross-compilation.

### Anti-Pattern 3: Using ephemeral EC2 public IP for DNS

**What people do:** Copy EC2 public IP to GoDaddy A record without allocating Elastic IP.
**Why it's wrong:** Public IP changes on every stop/start. DNS becomes stale immediately.
**Do this instead:** Allocate Elastic IP first, associate with instance, use Elastic IP in DNS.

### Anti-Pattern 4: In-place SCP over running binary

**What people do:** `scp binary ec2:/usr/local/bin/kiss-server` directly over the running file.
**Why it's wrong:** SCP may truncate the file in-place, causing SIGBUS in the running process.
**Do this instead:** SCP to `/tmp/`, then `stop → mv → start`.

## Sources

- GitHub Actions documentation — workflow syntax, permissions, branch protection
- AWS EC2 user guide — instance types, security groups, Elastic IP
- Amazon Linux 2023 documentation — systemd, iptables, EOL dates
- appleboy/scp-action and ssh-action GitHub repos
- softprops/action-gh-release GitHub repo

---
*Architecture research for: CI/CD ops & deployment, AWS EC2, GoDaddy DNS (v1.1)*
*Researched: 2026-03-10*
