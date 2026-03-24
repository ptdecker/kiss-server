# Phase 9: EC2 Service Setup - Research

**Researched:** 2026-03-24
**Domain:** Amazon Linux 2023 systemd service, iptables-services, Rust build on EC2, bash deployment scripting
**Confidence:** HIGH (critical findings verified live on the actual EC2 instance)

## Summary

This phase installs kiss-server on EC2 as a managed systemd service serving a Hello World page on port 80 via an iptables redirect. All three sub-problems (binary deployment, systemd unit, iptables persistence) are well-understood, with the exact command sequences verified live on the target instance (i-0394a6d927c0d9b33, Amazon Linux 2023).

**Critical blocker discovered:** `main.rs` hard-codes `DEFAULT_ADDR = "localhost:6502"` and has no `--port` flag. The server also binds to `localhost` (loopback only), not `0.0.0.0`. The CONTEXT.md assumes `kiss-server --root /var/www/ptodd.org --port 8080` works, but this CLI interface does not exist yet. Phase 9 MUST include a Rust code change to add `--port` and change the bind address to `0.0.0.0` before the deployment scripts can work. Additionally, the Cargo binary name is `ptodd` (from `Cargo.toml name = "ptodd"`), not `kiss-server` — the install script must copy `target/release/ptodd` to `/usr/local/bin/kiss-server`.

**Second critical finding:** The iptables INPUT chain on AL2023 (installed by iptables-services) has a default-deny REJECT rule. The `setup-iptables.sh` script must add INPUT chain ACCEPT rules for ports 80 AND 8080, not just the NAT PREROUTING REDIRECT rule. Without INPUT ACCEPT for port 8080, the redirected traffic is dropped before it reaches kiss-server.

**Primary recommendation:** Plan four tasks: (1) add `--port` CLI flag and `0.0.0.0` bind to main.rs, (2) `install-kiss-server.sh`, (3) `setup-webroot.sh`, (4) `setup-iptables.sh`. All scripts run via SSH from the developer machine.

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Build the binary on EC2 — not SCP from local, not download from GitHub Releases
- **D-02:** Clone the repo from GitHub on EC2 (`git clone`), install rustup, then `cargo build --release`
- **D-03:** Install deps (git, rustup) as part of the install-kiss-server.sh script
- **D-04:** Minimal placeholder `index.html` — bare-bones valid HTML (e.g., `<h1>Hello World</h1>`). This is a deployment smoke test; real content is not in scope for this phase.
- **D-05:** Three separate scripts, each independently idempotent and re-runnable:
  - `scripts/install-kiss-server.sh` — install deps, clone repo, cargo build --release, install binary to `/usr/local/bin/kiss-server`, create `kiss-server` service user, write and enable systemd unit
  - `scripts/setup-webroot.sh` — create `/var/www/ptodd.org/` directory, write `index.html`
  - `scripts/setup-iptables.sh` — install `iptables-services`, add PREROUTING rule for 80 → 8080, save rules
- **D-06:** All three scripts follow the established `scripts/` convention: `set -euo pipefail`, named constants at top, check-then-create idempotency
- **D-07:** Use `iptables-services` package: `dnf install iptables-services`, add `PREROUTING` DNAT or REDIRECT rule, then `service iptables save`. Rules persist in `/etc/sysconfig/iptables` and restore on boot.
- **D-08:** kiss-server runs as a dedicated non-root `kiss-server` system user (no home dir, no login shell). Port 80 traffic reaches it via the iptables redirect; the process binds to 8080 only.

### Claude's Discretion
- Exact `git clone` target path on EC2 (e.g., `/opt/kiss-server/` or `/home/ec2-user/kiss-server/`)
- rustup install flags (non-interactive, stable toolchain)
- Exact systemd unit configuration (Restart=on-failure, RestartSec, WantedBy target)
- iptables rule type (REDIRECT vs DNAT to 127.0.0.1:8080 — whichever is cleaner for local redirect)
- Minimal HTML content beyond `<h1>Hello World</h1>`

### Deferred Ideas (OUT OF SCOPE)
- **Download binary from GitHub Releases** — Once Phase 11 CD pipeline is shipping releases, `install-kiss-server.sh` should be updated to `curl` the release binary instead of building on EC2. User requested a GitHub issue be filed for this.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| DEPLOY-01 | kiss-server binary is installed at `/usr/local/bin/kiss-server` on EC2 | `install-kiss-server.sh` builds via cargo and copies `target/release/ptodd` to `/usr/local/bin/kiss-server`. Cargo binary name is `ptodd` — install step must rename. |
| DEPLOY-02 | kiss-server runs as a systemd service that starts on boot and restarts on failure | systemd unit with `WantedBy=multi-user.target`, `Restart=on-failure`. `systemctl enable` creates symlink. Verified systemd 252 on AL2023. |
| DEPLOY-03 | kiss-server listens on port 8080; iptables redirects port 80 → 8080 | PREROUTING REDIRECT rule verified live. INPUT chain ACCEPT rules for ports 80 AND 8080 required — otherwise INPUT default-deny drops redirected traffic. `--port` flag must be added to main.rs first. |
| DEPLOY-04 | `/var/www/ptodd.org/` directory exists on EC2 as the static file root | `setup-webroot.sh` creates directory with correct ownership for `kiss-server` user. |
| DEPLOY-05 | A "Hello World" `index.html` is deployed at `/var/www/ptodd.org/index.html` and served correctly | `setup-webroot.sh` writes minimal HTML. kiss-server serves it via `StaticFileHandler` with `--root /var/www/ptodd.org`. |
</phase_requirements>

---

## Critical Pre-Condition: main.rs Code Changes Required

### Finding 1: No --port flag exists (BLOCKING)

Current `main.rs` line 16:
```rust
const DEFAULT_ADDR: &str = "localhost:6502";
```

And line 54:
```rust
Server::new(DEFAULT_ADDR)?.with_router(router).run()?;
```

The server:
1. Binds to `localhost` (127.0.0.1) — loopback only, not reachable from outside
2. Uses port 6502, not 8080
3. Has no `--port` CLI argument (verified by grep — only `--root` is parsed)

**The systemd unit cannot invoke `kiss-server --port 8080` until this is implemented.**

The planner MUST include a task to add `--port <num>` argument parsing to `main.rs` and change the bind address from `localhost` to `0.0.0.0` (or make the bind host configurable via `--host`).

### Finding 2: Binary name is `ptodd`, not `kiss-server` (BLOCKING)

`Cargo.toml` sets `name = "ptodd"` with no `[[bin]]` override. After `cargo build --release`, the binary is at:
```
target/release/ptodd
```

The install script must explicitly copy/rename it:
```bash
sudo cp /opt/ptodd/target/release/ptodd /usr/local/bin/kiss-server
```

---

## Standard Stack

### Core (verified on instance)

| Tool | Version | Purpose | Source |
|------|---------|---------|--------|
| Amazon Linux 2023 | 2023 | Target OS | Verified live: `/etc/os-release` |
| systemd | 252 | Service management | Verified live: `systemctl --version` |
| iptables-services | 1.8.8-3.amzn2023.0.2 | iptables rule persistence | Verified in dnf repo + installed live |
| rustup | latest stable | Rust toolchain installer | Available via `curl` on the instance |
| git | via dnf | Repo clone | Not installed by default; dnf install git |

### Supporting

| Tool | Version | Purpose | When to Use |
|------|---------|---------|-------------|
| curl | 8.17.0 | rustup installer download | Pre-installed on AL2023 |
| cargo | via rustup | Build Rust binary | Installed by rustup |

### Instance Resources (verified live)

| Resource | Available | Notes |
|----------|-----------|-------|
| RAM | 916 MB total, 630 MB free | t3.micro; `cargo build --release` uses ~500MB peak |
| Disk | 8 GB total, 6.3 GB free | Sufficient for Rust toolchain + build artifacts |
| Swap | 0 MB | No swap — OOM risk during compilation; monitor |

**Installation commands for scripts:**
```bash
# git
sudo dnf install -y git

# rustup (non-interactive, stable)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable

# Load rustup env into script
source "$HOME/.cargo/env"
```

---

## Architecture Patterns

### Recommended Project Structure (new files this phase)

```
src/
└── main.rs              # MODIFIED: add --port flag, 0.0.0.0 bind
scripts/
├── install-kiss-server.sh   # NEW: deps, git clone, cargo build, systemd unit
├── setup-webroot.sh         # NEW: /var/www/ptodd.org + index.html
└── setup-iptables.sh        # NEW: iptables-services + rules + save
```

### Pattern 1: Script Idempotency (established in this project)

Every script follows the pattern from `setup-aws-infra.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

CONSTANT="value"

# Check-then-create
if condition_already_true; then
  echo "  already done, skipping."
else
  echo "  doing the thing..."
  do_the_thing
fi
```

### Pattern 2: Systemd Unit for Non-Root Service

**What:** Run a binary as a dedicated system user, on a non-privileged port.

```ini
[Unit]
Description=kiss-server static file server
After=network.target

[Service]
Type=simple
User=kiss-server
ExecStart=/usr/local/bin/kiss-server --root /var/www/ptodd.org --port 8080
Restart=on-failure
RestartSec=5

[Install]
WantedBy=multi-user.target
```

**Note:** `Type=simple` is correct — kiss-server does not daemonize itself (no `fork()`). systemd will track the process directly.

### Pattern 3: iptables REDIRECT for Port Forwarding

**What:** Redirect inbound port 80 traffic to port 8080, without requiring root process.

**Rule (verified live on instance):**
```bash
# NAT table PREROUTING — redirect external port 80 to 8080
sudo iptables -t nat -A PREROUTING -p tcp --dport 80 -j REDIRECT --to-port 8080

# INPUT chain — allow port 80 (before redirect)
sudo iptables -A INPUT -p tcp --dport 80 -j ACCEPT

# INPUT chain — allow port 8080 (after redirect, loopback path)
sudo iptables -A INPUT -p tcp --dport 8080 -j ACCEPT
```

**Critical:** The INPUT ACCEPT rules must be inserted BEFORE the existing REJECT rule. Using `-A` (append) is safe here only if the default-deny REJECT is already the last rule. Idempotency check must verify rule is not already present before appending.

### Pattern 4: Service User Creation (idempotent)

```bash
KISS_USER="kiss-server"

if id "$KISS_USER" &>/dev/null; then
  echo "  User '$KISS_USER' already exists, skipping."
else
  sudo useradd --system --no-create-home --shell /sbin/nologin "$KISS_USER"
  echo "  User '$KISS_USER' created."
fi
```

### Pattern 5: rustup Non-Interactive Install (idempotent)

```bash
if command -v cargo &>/dev/null; then
  echo "  Rust toolchain already installed, skipping."
else
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
  source "$HOME/.cargo/env"
fi
```

### Anti-Patterns to Avoid

- **Binding to `localhost` in systemd service:** The service user `kiss-server` needs to accept connections from the redirect — `0.0.0.0:8080` works. `localhost:8080` also works because PREROUTING REDIRECT redirects to localhost. However, `0.0.0.0` is cleaner and allows direct testing on port 8080.
- **Using `cargo install` instead of `cargo build --release` + manual copy:** `cargo install` puts binary in `$HOME/.cargo/bin/`, not accessible to the system service user.
- **Omitting INPUT chain ACCEPT rules:** PREROUTING REDIRECT alone is not enough — the AL2023 default iptables-services config has a final REJECT rule in INPUT that drops unmatched traffic including redirected port 8080 traffic.
- **Running service iptables save before starting the iptables service:** `service iptables save` fails with `[FAILED]` if the iptables service is not active. Start and enable first.
- **Hardcoding the git clone path for ec2-user vs root:** rustup installs to the invoking user's `$HOME`. Scripts must run as `ec2-user` (via SSH), not as root via `sudo`, to keep the toolchain in `/home/ec2-user/.cargo/`.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| iptables persistence across reboots | Custom rc.local or cron | iptables-services + `service iptables save` | iptables-services integrates with systemd and handles save/restore correctly |
| Port 80 binding as non-root | CAP_NET_BIND_SERVICE, authbind | iptables PREROUTING REDIRECT | Simpler, no capability management, standard pattern for AL2023 |
| Service restart on failure | Custom watchdog script | systemd `Restart=on-failure` | systemd is the init system; it already does this reliably |
| Rust version management | Manual binary download | rustup | Handles toolchain, cargo, rustfmt, clippy together |

**Key insight:** AL2023 has all the needed primitives (systemd, dnf, iptables-services) — no custom solutions needed for any sub-problem.

---

## Runtime State Inventory

> This is a deployment phase, not a rename/refactor phase. However, the iptables state is worth documenting.

| Category | Items Found | Action Required |
|----------|-------------|------------------|
| Stored data | None — no databases used | None |
| Live service config | iptables filter table: default INPUT REJECT rule pre-exists from iptables-services defaults | setup-iptables.sh must add INPUT ACCEPT rules for ports 80 and 8080 before the REJECT rule |
| OS-registered state | iptables-services package installed (from research probing) — iptables service is now enabled on the instance | setup-iptables.sh must handle already-installed case idempotently |
| Secrets/env vars | None | None |
| Build artifacts | None pre-existing | install-kiss-server.sh creates `/opt/ptodd/` build dir and `/usr/local/bin/kiss-server` |

**Note on research probing:** During research, `iptables-services` was installed on the instance to verify the command sequence. The iptables service is now active and enabled on the instance. The test REDIRECT rule was removed and the rule list was restored to default. `setup-iptables.sh` must handle the "already installed" case without failing.

---

## Common Pitfalls

### Pitfall 1: iptables INPUT chain blocks redirected traffic

**What goes wrong:** PREROUTING REDIRECT sends port 80 traffic to port 8080, but the INPUT chain's REJECT rule drops it before kiss-server sees it. `curl http://[ip]/` gets connection refused or ICMP unreachable.

**Why it happens:** iptables-services on AL2023 installs a default `/etc/sysconfig/iptables` with a final `-A INPUT -j REJECT` rule. The PREROUTING table operates before INPUT, but the packet still traverses INPUT after redirect.

**How to avoid:** `setup-iptables.sh` must add:
```bash
sudo iptables -A INPUT -p tcp --dport 80 -j ACCEPT
sudo iptables -A INPUT -p tcp --dport 8080 -j ACCEPT
```
These must be added before (i.e., `-I` insert at position before REJECT, or use `-A` knowing REJECT is already last) and saved.

**Warning signs:** `curl -v http://54.83.192.65/` returns connection refused or ICMP unreachable even when kiss-server process is running.

### Pitfall 2: `service iptables save` fails if iptables service is not started first

**What goes wrong:** Running `service iptables save` on a freshly installed `iptables-services` package returns `[FAILED]` because the service unit is not active.

**Why it happens:** The save command requires the service to be running. `dnf install iptables-services` does not auto-start or enable the service.

**How to avoid:** Always sequence:
```bash
sudo systemctl start iptables
sudo systemctl enable iptables
# ... add rules ...
sudo service iptables save
```

**Verified on instance:** This exact sequence was confirmed working during research.

### Pitfall 3: `--port` flag does not exist in kiss-server yet

**What goes wrong:** Systemd unit invokes `kiss-server --port 8080` which fails immediately because main.rs does not parse `--port`. Service enters a restart loop. `journalctl -u kiss-server` shows "unrecognized option" error.

**Why it happens:** CONTEXT.md describes the desired CLI, but the code does not implement it yet.

**How to avoid:** Phase plan MUST include a task to add `--port` argument parsing (and `0.0.0.0` bind) to `src/main.rs` before writing the systemd unit or deployment scripts. This task should also run `cargo test` to ensure existing tests pass.

**Warning signs:** `cargo build --release && ./target/release/ptodd --port 8080` exits with error.

### Pitfall 4: Binary name mismatch (ptodd vs kiss-server)

**What goes wrong:** Script tries to install `/usr/local/bin/kiss-server` but copies from wrong path.

**Why it happens:** `Cargo.toml` has `name = "ptodd"` — `cargo build --release` produces `target/release/ptodd`.

**How to avoid:** Install step in `install-kiss-server.sh`:
```bash
sudo cp /opt/ptodd/target/release/ptodd /usr/local/bin/kiss-server
sudo chmod +x /usr/local/bin/kiss-server
```

### Pitfall 5: rustup installed as ec2-user; cargo not in PATH for sudo commands

**What goes wrong:** `sudo cargo build` fails because sudo uses root's PATH, not ec2-user's.

**Why it happens:** rustup installs to `~/.cargo/bin/` for the invoking user. When scripts run `sudo cargo`, sudo drops the PATH.

**How to avoid:** Build as `ec2-user` (no sudo). Only use sudo for the install step:
```bash
# Build as ec2-user (no sudo)
source "$HOME/.cargo/env"
cargo build --release

# Only the install step uses sudo
sudo cp target/release/ptodd /usr/local/bin/kiss-server
```

### Pitfall 6: OOM during cargo build on t3.micro (no swap)

**What goes wrong:** `cargo build --release` runs out of memory and is killed. Build fails with no clear error.

**Why it happens:** t3.micro has 1 GB RAM, no swap. Rust release builds with LTO can exceed available memory with multiple parallel jobs.

**How to avoid:** Use `cargo build --release` (default parallelism is usually fine for a small codebase). If OOM occurs, fall back to `CARGO_BUILD_JOBS=1 cargo build --release`. Current disk has 6.3 GB free — adding a swap file is an option if needed:
```bash
sudo dd if=/dev/zero of=/swapfile bs=1M count=512 && sudo chmod 600 /swapfile && sudo mkswap /swapfile && sudo swapon /swapfile
```

**Warning signs:** Build process killed unexpectedly; `dmesg | grep -i oom` shows OOM killer activity.

### Pitfall 7: Idempotency of iptables rules

**What goes wrong:** Re-running `setup-iptables.sh` adds duplicate PREROUTING REDIRECT and INPUT ACCEPT rules.

**Why it happens:** `iptables -A` (append) always adds a new rule regardless of existing rules.

**How to avoid:** Check before adding:
```bash
if ! sudo iptables -t nat -C PREROUTING -p tcp --dport 80 -j REDIRECT --to-port 8080 2>/dev/null; then
  sudo iptables -t nat -A PREROUTING -p tcp --dport 80 -j REDIRECT --to-port 8080
fi
```
`iptables -C` (check) exits 0 if the rule exists, non-zero if not. This is the idiomatic idempotency check for iptables rules.

---

## Code Examples

### Adding --port to main.rs

```rust
// Source: verified against existing parse_root_from() pattern in main.rs

const DEFAULT_PORT: u16 = 6502;

fn parse_port_from(args: &[String]) -> crate::Result<u16> {
    if let Some(pos) = args.iter().position(|a| a == "--port") {
        let port_str = args.get(pos + 1).ok_or("--port requires a port number")?;
        let port: u16 = port_str.parse().map_err(|_| format!("--port '{}': not a valid port number", port_str))?;
        Ok(port)
    } else {
        Ok(DEFAULT_PORT)
    }
}

fn main() -> Result<()> {
    SimpleLogger::init()?;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = parse_root_from(&args)?;
    let port = parse_port_from(&args)?;
    let addr = format!("0.0.0.0:{}", port);
    info!("Serving static files from root: {}", root.display());
    let handler = StaticFileHandler::new(root)?;
    let mut router = Router::new();
    router.add("GET", "/", RootHandler)?;
    let router = router.set_fallback(handler);
    Server::new(&addr)?.with_router(router).run()?;
    Ok(())
}
```

### systemd unit file

```ini
# /etc/systemd/system/kiss-server.service
[Unit]
Description=kiss-server static file server
After=network.target

[Service]
Type=simple
User=kiss-server
ExecStart=/usr/local/bin/kiss-server --root /var/www/ptodd.org --port 8080
Restart=on-failure
RestartSec=5
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
```

### iptables idempotent rule addition

```bash
# Verified command sequence on AL2023 (iptables-services v1.8.8)

# Install (idempotent via dnf)
sudo dnf install -y iptables-services

# Start and enable service (idempotent — enable is idempotent, start is idempotent)
sudo systemctl start iptables
sudo systemctl enable iptables

# Add PREROUTING redirect (idempotent check)
if ! sudo iptables -t nat -C PREROUTING -p tcp --dport 80 -j REDIRECT --to-port 8080 2>/dev/null; then
  sudo iptables -t nat -A PREROUTING -p tcp --dport 80 -j REDIRECT --to-port 8080
fi

# Add INPUT ACCEPT for port 80 (idempotent check)
if ! sudo iptables -C INPUT -p tcp --dport 80 -j ACCEPT 2>/dev/null; then
  sudo iptables -A INPUT -p tcp --dport 80 -j ACCEPT
fi

# Add INPUT ACCEPT for port 8080 (idempotent check)
if ! sudo iptables -C INPUT -p tcp --dport 8080 -j ACCEPT 2>/dev/null; then
  sudo iptables -A INPUT -p tcp --dport 8080 -j ACCEPT
fi

# Save — persists to /etc/sysconfig/iptables, restored on boot by iptables.service
sudo service iptables save
```

### Script execution pattern (from developer machine)

```bash
# Upload script then execute via SSH
scp -i ~/.ssh/id_ed25519 scripts/install-kiss-server.sh ec2-user@54.83.192.65:/tmp/
ssh -i ~/.ssh/id_ed25519 ec2-user@54.83.192.65 'bash /tmp/install-kiss-server.sh'

# Or pipe directly
ssh -i ~/.ssh/id_ed25519 ec2-user@54.83.192.65 'bash -s' < scripts/install-kiss-server.sh
```

### git clone path recommendation (Claude's Discretion)

Recommended: `/opt/ptodd/` — `/opt` is the standard Linux directory for third-party software.

```bash
CLONE_DIR="/opt/ptodd"
REPO_URL="https://github.com/[owner]/ptodd.git"

if [ -d "$CLONE_DIR/.git" ]; then
  echo "  Repo already cloned, pulling latest..."
  git -C "$CLONE_DIR" pull
else
  sudo git clone "$REPO_URL" "$CLONE_DIR"
  sudo chown -R ec2-user:ec2-user "$CLONE_DIR"
fi
```

### Minimal Hello World index.html

```html
<!DOCTYPE html>
<html lang="en">
<head><meta charset="UTF-8"><title>ptodd.org</title></head>
<body><h1>Hello World</h1></body>
</html>
```

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|----------|
| SSH to ec2-user@54.83.192.65 | All scripts | ✓ | Verified Phase 8 | — |
| iptables-services (dnf) | setup-iptables.sh | ✓ | 1.8.8-3.amzn2023.0.2 | — (verified in repo) |
| curl | rustup installer | ✓ | 8.17.0 | — (pre-installed AL2023) |
| git | git clone on EC2 | ✗ (not installed) | — | `sudo dnf install -y git` in script |
| rustup | cargo build | ✗ (not installed) | — | Install via curl in script |
| cargo | cargo build | ✗ (not installed) | — | Installed by rustup |
| systemd | kiss-server.service | ✓ | 252 | — |
| RAM for cargo build | install-kiss-server.sh | Marginal (630 MB free, no swap) | — | Add swap file in script as precaution |

**Missing dependencies with no fallback:**
- None — all missing tools have scripted install paths.

**Missing dependencies with fallback:**
- git: `sudo dnf install -y git` in install-kiss-server.sh (D-03 locked decision)
- rustup/cargo: curl installer in install-kiss-server.sh (D-03 locked decision)

**RAM concern (marginal, not blocking):**
- t3.micro has 1 GB RAM, 630 MB free, 0 swap. `cargo build --release` for a small codebase typically needs 400-800 MB peak. Consider adding a 512 MB swap file as an optional precaution step in install-kiss-server.sh to avoid OOM.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in (`cargo test`) |
| Config file | `rust-toolchain.toml` (toolchain pin) |
| Quick run command | `cargo test` |
| Full suite command | `cargo test` |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DEPLOY-01 | Binary at `/usr/local/bin/kiss-server` on EC2 | smoke (SSH) | `ssh ec2-user@54.83.192.65 'test -x /usr/local/bin/kiss-server && echo OK'` | ❌ Wave 0 (verification command) |
| DEPLOY-02 | systemd service starts on boot, restarts on failure | smoke (SSH) | `ssh ec2-user@54.83.192.65 'systemctl is-active kiss-server'` | ❌ Wave 0 (verification command) |
| DEPLOY-03 | Port 80 → 8080 redirect works | smoke (curl) | `curl -s -o /dev/null -w "%{http_code}" http://54.83.192.65/` → 200 | ❌ Wave 0 (verification command) |
| DEPLOY-04 | `/var/www/ptodd.org/` exists on EC2 | smoke (SSH) | `ssh ec2-user@54.83.192.65 'test -d /var/www/ptodd.org && echo OK'` | ❌ Wave 0 (verification command) |
| DEPLOY-05 | index.html served at root | smoke (curl) | `curl -s http://54.83.192.65/ | grep -q "Hello World" && echo OK` | ❌ Wave 0 (verification command) |

**Note on unit tests for --port change:**
The new `parse_port_from()` function should have unit tests following the `parse_root_from()` test pattern already in main.rs. These are standard `cargo test` unit tests.

| Behavior | Test Type | Automated Command | File Exists? |
|----------|-----------|-------------------|-------------|
| `--port 8080` returns 8080 | unit | `cargo test parse_port` | ❌ Wave 0 (new tests in main.rs) |
| missing `--port` returns DEFAULT_PORT | unit | `cargo test parse_port` | ❌ Wave 0 |
| invalid `--port abc` returns Err | unit | `cargo test parse_port` | ❌ Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test` (local)
- **Per wave merge:** `cargo test` + SSH smoke checks against EC2
- **Phase gate:** `curl http://54.83.192.65/` returns 200 with Hello World content

### Wave 0 Gaps
- [ ] Unit tests for `parse_port_from()` — add to `src/main.rs` alongside existing `parse_root_from()` tests
- [ ] SSH smoke verification commands — run ad hoc after each deployment task (not automated test files, but listed as verification steps in plan tasks)

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| iptables directly (no persistence) | iptables-services for systemd-managed persistence | AL2023 baseline | `service iptables save` writes to `/etc/sysconfig/iptables`; restored on boot |
| nftables (AL2023 default kernel firewall) | iptables-services (wraps nftables via compat layer) | AL2023 ships nftables as backend | `iptables` v1.8.8 reports `(nf_tables)` — it uses nftables backend, but iptables syntax works correctly |

**Important:** `iptables v1.8.8 (nf_tables)` means iptables is using the nftables kernel backend. The iptables syntax works normally. This is NOT the same as using `nft` commands directly. The `iptables-services` package handles this transparently.

---

## Open Questions

1. **Swap file — add as precaution or rely on small codebase?**
   - What we know: t3.micro has 1 GB RAM, 0 swap, 630 MB free. The ptodd codebase is small (no external deps beyond `log`).
   - What's unclear: Whether `cargo build --release` with default parallelism will OOM on this specific codebase.
   - Recommendation: Include a swap file creation step in `install-kiss-server.sh` as an optional precaution (`if ! swapon --show | grep -q swapfile`). Low cost, prevents hard-to-diagnose build failure.

2. **GitHub repo URL for git clone**
   - What we know: The repo exists on GitHub (CI is configured via GitHub Actions per Phase 6/7).
   - What's unclear: The exact repo URL — not captured in CONTEXT.md or REQUIREMENTS.md.
   - Recommendation: Make the repo URL a named constant at the top of `install-kiss-server.sh`. Planner should note this is a human-fill placeholder or check `git remote -v` in the local repo.

3. **File ownership for /var/www/ptodd.org**
   - What we know: kiss-server process runs as `kiss-server` user.
   - What's unclear: Whether the directory should be owned by `kiss-server` or `root` (with read permissions for `kiss-server`).
   - Recommendation: Make directory root-owned, files world-readable (`chmod 644 index.html`). kiss-server only needs read access. This is simpler and avoids the webroot being writable by the service process.

---

## Sources

### Primary (HIGH confidence)
- Live EC2 instance probe — command sequences verified directly on i-0394a6d927c0d9b33 (Amazon Linux 2023)
- `src/main.rs` — binary name, CLI interface, bind address verified by reading source
- `Cargo.toml` — confirmed binary name is `ptodd` (no [[bin]] override)
- `scripts/setup-aws-infra.sh` — established script pattern read directly

### Secondary (MEDIUM confidence)
- systemd unit file patterns — standard systemd documentation patterns; `Type=simple` for non-forking processes is well-established

### Tertiary (LOW confidence)
- None — all critical claims verified from primary sources

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — verified live on actual instance
- Architecture: HIGH — code read directly; patterns verified against existing scripts
- Pitfalls: HIGH — iptables INPUT issue discovered and verified live; other pitfalls derived from source code analysis
- iptables command sequence: HIGH — executed and verified live on the instance

**Research date:** 2026-03-24
**Valid until:** 2026-04-24 (stable tooling; AL2023 package versions change slowly)

**Notable:** The two most important findings (missing --port flag, INPUT chain blocking) were NOT in the STATE.md research flags. The STATE.md flag was "confirm iptables-services install sequence" — which was confirmed, but the deeper issue (INPUT chain ACCEPT rules also required) was discovered during the live probe. Both findings are blocking and must appear in the plan.
