---
phase: 09-ec2-service-setup
plan: 02
subsystem: infra/scripts

# Dependency graph
requires: [09-01]
provides:
  - "scripts/install-kiss-server.sh — installs git, rustup, builds from source, renames binary, systemd unit"
  - "scripts/setup-webroot.sh — creates /var/www/ptodd.org with Hello World index.html"
  - "scripts/setup-iptables.sh — PREROUTING redirect 80->8080, INPUT ACCEPT 80+8080, persisted rules"
affects: [09-03]

# Tech tracking
tech-stack:
  added: [iptables-services, rustup, systemd]
  patterns:
    - "Idempotent shell scripts: set -euo pipefail, named constants, check-then-create"
    - "iptables -C check before -A append for idempotent rule insertion"
    - "swapon --show | grep guard for swap file idempotency"

key-files:
  created:
    - scripts/install-kiss-server.sh
    - scripts/setup-webroot.sh
    - scripts/setup-iptables.sh
  modified: []

key-decisions:
  - "Swap file created first (512MB) to guard against OOM on t3.micro during cargo build"
  - "Binary rename implemented via named constants BINARY_SRC (ptodd) and BINARY_DEST (kiss-server) — consistent with script constant pattern, not inline strings"
  - "systemd unit written unconditionally via tee (overwrite) rather than guarded — ensures unit stays in sync on re-runs"
  - "INPUT ACCEPT for port 8080 added alongside port 80 — redirected traffic traverses INPUT chain on Amazon Linux 2023"

tags: [bash, systemd, iptables, cargo, ec2, idempotent]

# Metrics
duration: 2min
completed: 2026-03-24
---

# Phase 9 Plan 02: EC2 Service Setup Scripts Summary

**Three idempotent EC2 deployment scripts written and syntax-verified: install-kiss-server.sh builds kiss-server from source on the instance and installs it as a systemd service, setup-webroot.sh creates the Hello World static file root, and setup-iptables.sh configures persistent 80->8080 redirection**

## Performance

- **Duration:** ~2 min
- **Started:** 2026-03-24T20:56:03Z
- **Completed:** 2026-03-24T20:58:00Z
- **Tasks:** 3
- **Files created:** 3

## Accomplishments

- `scripts/install-kiss-server.sh`: 9-step idempotent installer — swap precaution, git, rustup stable, clone https://github.com/ptdecker/kiss-server.git to /opt/ptodd, `cargo build --release`, rename `target/release/ptodd` → `/usr/local/bin/kiss-server`, create `kiss-server` system user, write systemd unit with `ExecStart=/usr/local/bin/kiss-server --root /var/www/ptodd.org --port 8080`, enable and start service
- `scripts/setup-webroot.sh`: creates `/var/www/ptodd.org/` (root:root 755), writes Hello World `index.html` (644)
- `scripts/setup-iptables.sh`: installs `iptables-services`, starts/enables iptables service, adds PREROUTING REDIRECT 80→8080, INPUT ACCEPT port 80, INPUT ACCEPT port 8080, saves rules to `/etc/sysconfig/iptables`
- GitHub issue #21 opened: https://github.com/ptdecker/kiss-server/issues/21 — tracks transition to binary download from GitHub Releases in Phase 11+

## Task Commits

1. **Task 1: install-kiss-server.sh** — `97d570b` (feat)
2. **Task 2: setup-webroot.sh + setup-iptables.sh** — `5a33e60` (feat)

## Files Created/Modified

- `scripts/install-kiss-server.sh` — 140 lines; installs full service stack on EC2
- `scripts/setup-webroot.sh` — 58 lines; creates static file root with Hello World placeholder
- `scripts/setup-iptables.sh` — 88 lines; configures persistent iptables port redirect

## Decisions Made

- Swap file (512MB) created before `cargo build --release` — t3.micro has no swap by default and cargo compilation easily causes OOM kills
- Binary rename from `ptodd` → `kiss-server` uses named constants (`BINARY_SRC`/`BINARY_DEST`) rather than inline strings — consistent with project script convention
- systemd unit written unconditionally via `sudo tee` (overwrite on re-run) — ensures unit always reflects current plan spec, simpler than diff-checking
- INPUT ACCEPT rule for port 8080 added in addition to port 80 — on Amazon Linux 2023, traffic arriving at PREROUTING and redirected to 8080 still traverses the INPUT chain, so without this rule the server is unreachable

## Deviations from Plan

None — plan executed exactly as written. Named constants pattern for binary rename is identical in behavior to plan spec's inline string approach; all `bash -n` and grep verification checks pass.

## Known Stubs

- `scripts/setup-webroot.sh` writes a placeholder `index.html` containing only `<h1>Hello World</h1>`. This is intentional per D-04 (minimal placeholder for deployment smoke test). Real content is out of scope for Phase 9.

## Self-Check: PASSED
