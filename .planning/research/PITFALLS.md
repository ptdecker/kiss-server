# Domain Pitfalls: Ops & Deployment (v1.1)

**Domain:** CI/CD pipeline, AWS EC2 deployment, DNS routing, GitHub releases for Rust binary
**Researched:** 2026-03-10
**Confidence:** HIGH — GitHub Actions official docs, AWS official docs, GoDaddy official help, community-verified patterns

---

## Critical Pitfalls

---

### Pitfall 1: Architecture Mismatch — CI Runner x86_64 vs EC2 arm64

**What goes wrong:** GitHub Actions `ubuntu-latest` runner is x86_64. If EC2 is `arm64` (e.g., t4g), the compiled binary fails to execute with `Exec format error`. Binary copies silently via SCP — failure only appears when `systemctl start` runs.

**Why it happens:** t4g instances are cheaper (~$3/mo vs $8.47/mo for t3.micro). Developers pick them without checking that the default CI runner builds x86_64.

**Consequences:** Service fails after every deployment. CI reports green while prod is broken.

**Prevention:**
1. Use `t3.micro` (x86_64) — matches `ubuntu-latest` runner; no cross-compilation
2. Add post-deploy health check: `systemctl is-active kiss-server || exit 1`
3. Document EC2 architecture alongside the workflow file

**Detection:**
- `systemctl status kiss-server` shows `Exec format error`
- `file /usr/local/bin/kiss-server` shows `ARM aarch64` on x86_64 host

**Phase:** AWS infrastructure phase — decide EC2 instance type before writing any workflow YAML.

---

### Pitfall 2: Branch Protection Locks You Out — Required Check Name Mismatch

**What goes wrong:** Branch protection requires a status check named `"ci"`. Workflow defines job named `build`. The required check never satisfies — every PR is permanently blocked.

A subtler variant: adding the branch protection rule before CI has run at least once. The check name won't appear in the autocomplete dropdown, leading to a typo that blocks all merges.

**Consequences:** All PRs blocked. If "Do not allow bypassing the above settings" is enabled for admins too, there is no recovery path short of GitHub support.

**Prevention:**
1. Name the workflow job with a stable, intentional name: `ci` — never rename it
2. Push a commit to any branch and let CI run before configuring branch protection
3. Use the autocomplete dropdown to select the check name — do NOT type manually
4. Never enable "Do not allow bypassing the above settings" as a solo developer — you need an admin escape hatch

**Detection:**
- PR shows "Required status check 'X' has not run" even after CI passes
- The passing check has a different name than the required check

**Phase:** Branch protection phase — always after first successful CI run.

---

### Pitfall 3: SSH Private Key CRLF — OpenSSH Rejects It

**What goes wrong:** EC2 SSH private key is pasted into a GitHub Secret with Windows line endings (`\r\n`). OpenSSH reports `Load key: invalid format`. Developer echoes the key for debugging, exposing it in workflow logs.

**Why it happens:** Key is copied from AWS Console on Windows or macOS, pasted into the GitHub Secret textarea. CRLF issue only surfaces when `ssh` runs.

**Consequences:** CD pipeline fails with cryptic error. Attempted debug exposes the private key in public logs.

**Prevention:**
1. Test the key locally: `ssh-keygen -y -f key.pem` — if this succeeds, format is correct
2. Strip CRLF before storing: `tr -d '\r' < key.pem` on a Unix system
3. In the workflow, write the key to a temp file without echoing:
   ```bash
   echo "${{ secrets.EC2_SSH_KEY }}" > /tmp/deploy_key
   chmod 600 /tmp/deploy_key
   ```
4. Never use `echo ${{ secrets.EC2_SSH_KEY }}` or `set -x` in a deploy workflow

**Detection:**
- `ssh` reports `Load key "/tmp/key": invalid format`
- GitHub Actions log shows base64-like content — key was echoed

**Phase:** CD pipeline phase — validate key format locally before storing as secret.

---

### Pitfall 4: EC2 Security Group Opens SSH to 0.0.0.0/0

**What goes wrong:** Security group inbound rule for port 22 allows `0.0.0.0/0`. Within minutes, automated scanners begin brute-force attempts.

Secondary mistake: opening port 8080 instead of port 80 — server runs on 8080 but Security Group opens the wrong port (or vice versa).

**Prevention:**
1. Restrict SSH source to your home/work IP: `x.x.x.x/32`. Update when your IP changes.
2. For CD pipeline SSH: add a second rule for GitHub Actions IP ranges, or accept any-source for SSH if your key is ED25519 (brute force is infeasible)
3. Security Group opens port **80** (public HTTP). Port 8080 stays closed — the iptables redirect happens inside the instance.
4. Verify Security Group port matches iptables redirect destination before testing DNS.

**Detection:**
- `curl http://[elastic-ip]/` connection refused — Security Group not open on port 80, or kiss-server not responding on 8080

**Phase:** AWS infrastructure phase — security group before DNS configuration.

---

### Pitfall 5: In-Place SCP Over Running Binary (SIGBUS Risk)

**What goes wrong:** CD script SCPs directly to `/usr/local/bin/kiss-server` while the service is running. SCP may truncate and rewrite the file in-place, corrupting the running process's memory pages.

**Why it happens:** SCP to the final path is simpler; developers don't realize `mv` is atomic but SCP is not.

**Consequences:** Intermittent SIGBUS crashes after deployment. Timing-dependent; hard to reproduce.

**Prevention:**
```bash
# SCP to temp path
scp kiss-server ec2:/tmp/kiss-server-new

# Atomic replacement
sudo systemctl stop kiss-server
sudo mv /tmp/kiss-server-new /usr/local/bin/kiss-server
sudo chmod +x /usr/local/bin/kiss-server
sudo systemctl start kiss-server
```
`mv` on the same filesystem is atomic on Linux. Always stop before mv.

**Detection:**
- `SIGBUS` in `journalctl -u kiss-server` immediately after deploy
- Service occasionally starts old binary version after deploy

**Phase:** CD pipeline phase — atomic replacement from the very first deploy script.

---

### Pitfall 6: `systemctl restart` Exits 0 Even When Service Immediately Crashes

**What goes wrong:** CD workflow runs `sudo systemctl restart kiss-server` and the step exits 0 (success). But the service crashes immediately after start (e.g., wrong `--root` path, bad binary). Pipeline reports success; prod is broken.

**Why it happens:** `systemctl restart` initiates the restart and exits — it does not wait for the service to stabilize.

**Consequences:** Green pipeline, broken prod. The "silent deploy failure" pattern.

**Prevention:**
Every CD deploy must end with:
```bash
systemctl is-active kiss-server || (journalctl -u kiss-server -n 30 && exit 1)
```
`systemctl is-active` exits non-zero if service is not `active`. This is the single highest-value safeguard in the entire pipeline.

**Detection:**
- `systemctl status kiss-server` shows `failed` or `inactive` after deploy
- `journalctl -u kiss-server -n 20` shows `--root is required` or similar

**Phase:** CD pipeline phase — health check is not optional.

---

## Moderate Pitfalls

---

### Pitfall 7: Cargo Cache Bloat Defeats CI Speed Gains

**What goes wrong:** Naively caching `~/.cargo` and `target/` causes cache to grow unboundedly. After 10-20 runs, restore/save time exceeds compilation time savings. GitHub has a 10 GB total cache limit.

**Prevention:**
1. Use `Swatinem/rust-cache@v2` — handles key construction, artifact cleanup, workspace crate exclusion
2. Set `CARGO_INCREMENTAL: "0"` — incremental artifacts are not reusable across runners
3. For a stdlib-only project, measure actual timing after 10 runs; full rebuild may be faster than bloated cache restore

**Phase:** CI pipeline phase — configure caching correctly from the start.

---

### Pitfall 8: Required Status Check Not in Branch Protection Autocomplete

**What goes wrong:** The autocomplete only shows checks that have run at least once. If you configure branch protection before CI has ever run, the dropdown is empty — manually typing the job name risks a typo.

**Prevention:**
1. Push a commit to a feature branch to trigger CI
2. Confirm the job ran successfully and appears in the commit's "Checks" tab
3. Only then configure branch protection — use autocomplete, not manual typing

**Phase:** Branch protection phase — always after first CI run.

---

### Pitfall 9: GoDaddy Default TTL Delays Testing by Hours

**What goes wrong:** GoDaddy default TTL is 600-3600 seconds. Some ISPs ignore TTL and cache for 24-48 hours. After pointing the domain to EC2, `curl http://ptodd.org` returns `Connection refused` — but it's DNS propagation, not EC2 failure.

Secondary: using CNAME for the apex domain `@`. DNS spec (RFC 1912) prohibits CNAME at zone apex. GoDaddy correctly rejects it.

**Prevention:**
1. Use A record (not CNAME) for `@` → Elastic IP
2. Add CNAME `www` → `@` simultaneously
3. Lower TTL to 300s before making changes; wait one full cycle before changing
4. Test DNS resolution: `dig @8.8.8.8 ptodd.org A` (bypasses ISP cache)
5. Use `https://dnschecker.org` to verify propagation across multiple resolvers

**Detection:**
- `dig @8.8.8.8 ptodd.org` returns new IP but `curl http://ptodd.org` still hits old IP — ISP cache
- GoDaddy rejects CNAME on `@` — use A record instead

**Phase:** DNS configuration phase — allow 1 hour after change before debugging.

---

### Pitfall 10: GitHub Release Asset Upload Fails with 403

**What goes wrong:** CD workflow creates a GitHub Release and uploads the binary. Step fails with HTTP 403. The `GITHUB_TOKEN` has default read-only permissions on new repositories.

Secondary: using `actions/upload-artifact@v3` — deprecated January 30, 2025.

**Prevention:**
```yaml
jobs:
  deploy:
    permissions:
      contents: write    # Required for release asset upload
```
Use `softprops/action-gh-release@v2` (not v1), `actions/upload-artifact@v4` (not v3).

**Detection:**
- Workflow fails with `HTTP 403 Forbidden` at asset upload
- GitHub release exists but shows "0 assets"

**Phase:** CD pipeline phase — set permissions before writing the release step.

---

### Pitfall 11: No Elastic IP — EC2 Public IP Changes on Stop/Start

**What goes wrong:** EC2 public IP is used in the DNS A record. Instance is stopped for maintenance. It gets a new public IP. DNS record is stale. `ptodd.org` goes down until the A record is manually updated and DNS re-propagates.

**Prevention:** Allocate an Elastic IP as the very first infrastructure step. Everything else depends on it being stable.

**Detection:**
- `ptodd.org` resolves to old IP after instance restart
- SSH from CD pipeline fails with `Host key verification failed` (different IP)

**Phase:** AWS infrastructure phase — Elastic IP before anything else.

---

### Pitfall 12: `systemd` Unit File Missing `--root` Argument

**What goes wrong:** `ExecStart=/usr/local/bin/kiss-server` without `--root`. Server starts, immediately exits with error, systemd marks it `failed`. After rapid restart failures, systemd applies rate limit and stops retrying.

**Prevention:**
```ini
ExecStart=/usr/local/bin/kiss-server --root /var/www/ptodd.org --port 8080
```
Write the unit file and test it manually before automating with CD.

**Detection:**
- `systemctl status kiss-server` shows `failed`
- `journalctl -u kiss-server -n 10` shows `--root is required` or `No such file or directory`

**Phase:** EC2 service setup phase — unit file and deploy script written and tested together.

---

## Minor Pitfalls

---

### Pitfall 13: `cargo clippy` Fails on New Lint in Updated Stable Toolchain

**What goes wrong:** CI runs `cargo clippy -- -D warnings`. Codebase is lint-free locally. A newer stable toolchain on the CI runner introduces a new lint that the local toolchain didn't have. CI fails, blocking all PRs.

**Prevention:** Add `rust-toolchain.toml` to the repo root pinning a specific stable version. Accept toolchain updates as explicit maintenance tasks.

**Phase:** CI pipeline phase — add `rust-toolchain.toml` with the first CI setup.

---

### Pitfall 14: `www.ptodd.org` Unreachable — Missing CNAME

**What goes wrong:** A record set for `@` (ptodd.org). Visitors typing `www.ptodd.org` get `NXDOMAIN` — no DNS record for `www`.

**Prevention:** Add `www CNAME → @` at the same time as the A record. Test both:
```bash
curl http://ptodd.org/
curl http://www.ptodd.org/
```

**Phase:** DNS configuration phase.

---

## Phase-Specific Warning Summary

| Phase Topic | Key Pitfall | Mitigation |
|-------------|-------------|------------|
| CI workflow | Cargo cache bloat | `Swatinem/rust-cache@v2` + `CARGO_INCREMENTAL=0` |
| CI workflow | Toolchain version drift | Pin in `rust-toolchain.toml` |
| Branch protection | Check name mismatch → lockout | Run CI once first; use autocomplete; no admin bypass disable |
| Branch protection | Check not in autocomplete | Push commit → wait for run → then configure |
| AWS infrastructure | EC2 arm64 arch mismatch | Use t3.micro (x86_64) to match CI runner |
| AWS infrastructure | Elastic IP not allocated | Allocate first — DNS depends on it |
| AWS infrastructure | SSH open to 0.0.0.0/0 | Restrict to your IP `/32` |
| EC2 service setup | Wrong port / missing --root in unit file | Write unit file + test manually before CD |
| EC2 service setup | In-place SCP race condition | Atomic: SCP to /tmp, stop, mv, start |
| CD pipeline | `systemctl restart` exits 0 on crash | Always follow with `systemctl is-active` check |
| CD pipeline | SSH key CRLF rejected | Test with `ssh-keygen -y -f` before storing as secret |
| CD pipeline | GitHub Release 403 | Add `permissions: contents: write` to job |
| DNS configuration | TTL delays testing | Lower TTL to 300s before change; test with `dig @8.8.8.8` |
| DNS configuration | CNAME at apex rejected | Use A record for `@`; CNAME only for `www` |
| DNS configuration | `www` unreachable | Add CNAME `www → @` with the A record |

## Critical Cross-Phase Integration

### The "Green CI, Broken Prod" Pattern

The most dangerous failure mode: CI reports success, CD exits 0, service is not running the new binary.

Root causes:
- `systemctl restart` exits 0 even on immediate crash
- SCP exits 0 even if binary is wrong architecture
- Deploy script checks wrong signal

**The mandatory safeguard** (add to every CD workflow):
```bash
systemctl is-active kiss-server || (journalctl -u kiss-server -n 30 && exit 1)
```

### Architecture Decision Matrix

| Environment | Architecture | Notes |
|-------------|-------------|-------|
| macOS dev (M-series) | arm64 | Binary never deployed from here |
| GitHub Actions `ubuntu-latest` | x86_64 | This is what gets deployed to EC2 |
| EC2 `t3.micro` | x86_64 | Matches runner — no cross-compilation |
| EC2 `t4g.micro` | arm64 | Requires cross-compilation — avoid for v1.1 |

---
*Pitfalls research for: CI/CD ops & deployment, AWS EC2, GoDaddy DNS (v1.1)*
*Researched: 2026-03-10*
