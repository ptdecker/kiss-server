# Phase 9: EC2 Service Setup - Context

**Gathered:** 2026-03-24
**Status:** Ready for planning

<domain>
## Phase Boundary

Install kiss-server on the EC2 instance (i-0394a6d927c0d9b33) as a managed systemd service serving a Hello World static page on port 80 via an iptables redirect from 80 → 8080. No Rust code changes. No DNS configuration (Phase 10). No CD pipeline (Phase 11).

</domain>

<decisions>
## Implementation Decisions

### Binary deployment method
- **D-01:** Build the binary on EC2 — not SCP from local, not download from GitHub Releases
- **D-02:** Clone the repo from GitHub on EC2 (`git clone`), install rustup, then `cargo build --release`
- **D-03:** Install deps (git, rustup) as part of the install-kiss-server.sh script

### Hello World content
- **D-04:** Minimal placeholder `index.html` — bare-bones valid HTML (e.g., `<h1>Hello World</h1>`). This is a deployment smoke test; real content is not in scope for this phase.

### Script structure
- **D-05:** Three separate scripts, each independently idempotent and re-runnable:
  - `scripts/install-kiss-server.sh` — install deps, clone repo, cargo build --release, install binary to `/usr/local/bin/kiss-server`, create `kiss-server` service user, write and enable systemd unit
  - `scripts/setup-webroot.sh` — create `/var/www/ptodd.org/` directory, write `index.html`
  - `scripts/setup-iptables.sh` — install `iptables-services`, add PREROUTING rule for 80 → 8080, save rules
- **D-06:** All three scripts follow the established `scripts/` convention: `set -euo pipefail`, named constants at top, check-then-create idempotency

### iptables persistence
- **D-07:** Use `iptables-services` package: `dnf install iptables-services`, add `PREROUTING` DNAT or REDIRECT rule, then `service iptables save`. Rules persist in `/etc/sysconfig/iptables` and restore on boot.

### Service user
- **D-08:** kiss-server runs as a dedicated non-root `kiss-server` system user (no home dir, no login shell). Port 80 traffic reaches it via the iptables redirect; the process binds to 8080 only.

### Claude's Discretion
- Exact `git clone` target path on EC2 (e.g., `/opt/kiss-server/` or `/home/ec2-user/kiss-server/`)
- rustup install flags (non-interactive, stable toolchain)
- Exact systemd unit configuration (Restart=on-failure, RestartSec, WantedBy target)
- iptables rule type (REDIRECT vs DNAT to 127.0.0.1:8080 — whichever is cleaner for local redirect)
- Minimal HTML content beyond `<h1>Hello World</h1>`

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

No external specs — requirements fully captured in decisions above and REQUIREMENTS.md (DEPLOY-01 through DEPLOY-05).

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `scripts/setup-aws-infra.sh` — the fully-formed reference script: `set -euo pipefail`, named constants, check-then-create pattern, `--profile kiss` on all aws CLI calls. New scripts must follow this pattern.
- `scripts/setup-branch-protection.sh` — second example of the script pattern using `gh` CLI
- `scripts/ci.sh` — simplest example in the pattern (just cargo commands)

### Established Patterns
- All `scripts/` files: idempotent, check-before-create, `set -euo pipefail`
- AWS CLI calls use `--profile kiss` and `--region us-east-1`
- Phase scripts are committed to the repo and run once during phase execution

### Integration Points
- `i-0394a6d927c0d9b33` — instance where scripts run (SSH: `ec2-user@54.83.192.65`)
- `~/.ssh/id_ed25519` — SSH key for connecting to EC2
- `--profile kiss` + `us-east-1` — AWS CLI context for any aws calls
- kiss-server binary CLI: `kiss-server --root /var/www/ptodd.org --port 8080` (inferred from src/main.rs)
- Phase 11 CD pipeline will replace the binary via SCP → stop → mv → start; systemd unit must support `systemctl stop kiss-server` cleanly

</code_context>

<specifics>
## Specific Ideas

- The `install-kiss-server.sh` script runs on the EC2 instance via SSH — the planner should include a local task (run from developer machine) that SSHs in and executes the remote scripts
- Phase 11 (CD pipeline) will switch from cargo-build-on-EC2 to downloading the compiled binary from GitHub Releases — create a GitHub issue to track this transition once CD is in place

</specifics>

<deferred>
## Deferred Ideas

- **Download binary from GitHub Releases** — Once Phase 11 CD pipeline is shipping releases, `install-kiss-server.sh` should be updated to `curl` the release binary instead of building on EC2. User requested a GitHub issue be filed for this.

</deferred>

---

*Phase: 09-ec2-service-setup*
*Context gathered: 2026-03-24*
