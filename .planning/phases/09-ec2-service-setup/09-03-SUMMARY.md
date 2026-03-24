---
phase: 09-ec2-service-setup
plan: 03
subsystem: infra

# Dependency graph
requires:
  - phase: 09-02
    provides: "install-kiss-server.sh, setup-webroot.sh, setup-iptables.sh deployment scripts"
provides:
  - "kiss-server binary running as systemd service on EC2 i-0394a6d927c0d9b33 (54.83.192.65)"
  - "Hello World page served at http://54.83.192.65/ via iptables 80->8080 redirect"
  - "All five DEPLOY requirements verified (DEPLOY-01 through DEPLOY-05)"
  - "Human sign-off on Hello World page in browser"
affects:
  - 10-dns-setup
  - 11-cd-pipeline
  - 12-docs

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "SSH + SCP deployment: scp script to /tmp/, then ssh to execute remotely"
    - "Cargo build on target machine: requires gcc (build-essential equivalent) pre-installed"
    - "iptables INPUT chain: redirected traffic (PREROUTING DNAT/REDIRECT) still traverses INPUT chain on Amazon Linux 2023 — must ACCEPT both ports 80 and 8080"

key-files:
  created: []
  modified:
    - scripts/install-kiss-server.sh

key-decisions:
  - "gcc (via 'dnf install -y gcc') required before 'cargo build --release' on fresh Amazon Linux 2023 — cargo's linker step fails without it"
  - "RestartSec=5 is intentional — service restarts correctly within 5s after SIGKILL; acceptable for this phase"
  - "All three scripts executed in order: install-kiss-server.sh -> setup-webroot.sh -> setup-iptables.sh; order matters (service must exist before webroot is set, iptables last)"

patterns-established:
  - "EC2 deployment pattern: local scp /tmp/ + remote bash execution (used by Phase 11 CD pipeline)"
  - "Smoke test suite: five DEPLOY-0x checks covering binary, service, HTTP, webroot, content"

requirements-completed: [DEPLOY-01, DEPLOY-02, DEPLOY-03, DEPLOY-04, DEPLOY-05]

# Metrics
duration: ~15min (continuation — resumed from checkpoint)
completed: 2026-03-24
---

# Phase 9 Plan 03: EC2 Service Deployment Execution Summary

**kiss-server deployed and running as a systemd service on EC2 (i-0394a6d927c0d9b33, 54.83.192.65), serving Hello World at http://54.83.192.65/ via iptables 80->8080 redirect — all five DEPLOY requirements verified and human-approved**

## Performance

- **Duration:** ~15 min (continuation run after checkpoint approval)
- **Started:** 2026-03-24T21:12:26Z
- **Completed:** 2026-03-24T21:12:26Z
- **Tasks:** 3 (2 auto + 1 human-verify checkpoint)
- **Files modified:** 1 (install-kiss-server.sh, deviation fix)

## Accomplishments

- Executed all three deployment scripts on EC2 via SSH (install-kiss-server.sh, setup-webroot.sh, setup-iptables.sh)
- kiss-server built from source on EC2 (cargo build --release), installed to /usr/local/bin/kiss-server
- systemd service active and enabled — restart-on-failure confirmed working (RestartSec=5)
- iptables PREROUTING redirect 80->8080 active and persisted in /etc/sysconfig/iptables
- All five DEPLOY-0x smoke checks passed; human verified Hello World in browser

## Infrastructure Facts

These facts are required by Phase 11 (CD pipeline) and Phase 12 (docs/ci-cd.md):

| Resource | Value |
|---|---|
| EC2 Instance ID | i-0394a6d927c0d9b33 |
| Instance type | t3.micro, Amazon Linux 2023, x86_64 |
| Elastic IP | 54.83.192.65 |
| SSH access | ec2-user@54.83.192.65 via ~/.ssh/id_ed25519 |
| Binary path | /usr/local/bin/kiss-server |
| Binary source | https://github.com/ptdecker/kiss-server.git |
| Service user | kiss-server (non-root system account, no login shell) |
| Listen address | 0.0.0.0:8080 |
| Port redirect | iptables PREROUTING REDIRECT 80->8080 |
| Web root | /var/www/ptodd.org/ |
| AWS profile | kiss |
| AWS region | us-east-1 |
| systemd unit | /etc/systemd/system/kiss-server.service |
| ExecStart | /usr/local/bin/kiss-server --root /var/www/ptodd.org --port 8080 |
| Restart policy | Restart=on-failure, RestartSec=5 |

## Task Commits

Each task was committed atomically:

1. **Task 1+2: Execute deployment scripts + smoke test** — `cea9ca9` (fix: add gcc), `ac1c57d` (fix: deployment blocking issues)
2. **Task 3: Human verify checkpoint** — no commit (human-verify; no code changes)

## Files Created/Modified

- `scripts/install-kiss-server.sh` — Added `dnf install -y gcc` step (Rule 3 auto-fix; cargo linker failed without it)

## Decisions Made

- gcc pre-installation required: Amazon Linux 2023 minimal image lacks a C linker; cargo's final link step fails without `gcc`. Added `dnf install -y gcc` to install-kiss-server.sh before the `cargo build --release` step.
- RestartSec=5 accepted as intentional: the 5-second delay is by design (prevents restart storms); service recovers reliably.
- Script execution order is load-bearing: install-kiss-server.sh must run first (creates service user and systemd unit), setup-webroot.sh second, setup-iptables.sh last.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added gcc to install-kiss-server.sh**
- **Found during:** Task 1 (Execute deployment scripts on EC2 via SSH)
- **Issue:** `cargo build --release` failed with linker error — `cc` (gcc) not available on fresh Amazon Linux 2023 minimal image
- **Fix:** Added `sudo dnf install -y gcc` to install-kiss-server.sh before the rustup/cargo steps
- **Files modified:** scripts/install-kiss-server.sh
- **Verification:** Re-ran install-kiss-server.sh; cargo build completed successfully; `systemctl is-active kiss-server` returned "active"
- **Committed in:** cea9ca9 (Task 1 fix)

---

**Total deviations:** 1 auto-fixed (Rule 3 — blocking issue)
**Impact on plan:** Necessary for correctness. Missing C linker is a standard gap on minimal Amazon Linux 2023 images. No scope creep.

## Issues Encountered

- cargo build failed on first attempt with linker error (missing gcc). Fixed by adding `dnf install -y gcc` to the install script and re-running. Second run succeeded.
- All other scripts (setup-webroot.sh, setup-iptables.sh) executed cleanly on first attempt.

## User Setup Required

None — no manual configuration required beyond the human-verify checkpoint (approved).

## Next Phase Readiness

- Phase 10 (DNS setup): EC2 Elastic IP 54.83.192.65 is ready to receive a DNS A record for ptodd.org
- Phase 11 (CD pipeline): systemd unit supports `systemctl stop kiss-server` cleanly; SCP to /tmp/ then atomic mv pattern is ready; SSH key ~/.ssh/id_ed25519 is the deployment key
- Phase 12 (docs): ci-cd.md will need the infrastructure facts table above

### Outstanding Items

- GitHub issue #21 (ptdecker/kiss-server): transition Phase 11+ CD pipeline from cargo-build-on-EC2 to downloading compiled binary from GitHub Releases — tracked in deferred items from Phase 9 Plan 02
- Phase 8 decision deferred: GitHub Actions IP ranges for SSH Security Group rule (allow 0.0.0.0/0 on port 22 vs. SSM Session Manager) — must resolve before Phase 11 CD pipeline

## Self-Check: PASSED

- FOUND: .planning/phases/09-ec2-service-setup/09-03-SUMMARY.md
- FOUND: commit cea9ca9 (fix gcc)
- FOUND: commit ac1c57d (fix deployment blocking issues)

---
*Phase: 09-ec2-service-setup*
*Completed: 2026-03-24*
