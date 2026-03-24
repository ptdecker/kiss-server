# Phase 10: DNS Configuration - Context

**Gathered:** 2026-03-24
**Status:** Ready for planning

<domain>
## Phase Boundary

Configure GoDaddy DNS so ptodd.org and www.ptodd.org resolve to Elastic IP 54.83.192.65 and return the Hello World page over HTTP. No infrastructure changes (Phase 8 complete). No deployment changes (Phase 9 complete). No TLS/HTTPS. No Route 53.

</domain>

<decisions>
## Implementation Decisions

### DNS Configuration Method
- **D-01:** Use the GoDaddy web UI (manual) — log into GoDaddy dashboard and set records by hand. No API script. The plan documents the exact steps: navigate to DNS management for ptodd.org, set the A record and CNAME.
- **D-02:** A record: `@` → `54.83.192.65` (Elastic IP from Phase 8)
- **D-03:** CNAME: `www` → `@` (so www.ptodd.org also resolves)

### Verification
- **D-04:** Commit `scripts/verify-dns.sh` — a terminal smoke test script that runs:
  - `dig ptodd.org` — confirm A record resolves to 54.83.192.65
  - `dig www.ptodd.org` — confirm CNAME resolves
  - `curl http://ptodd.org/` — confirm Hello World page returned
  - `curl http://www.ptodd.org/` — confirm Hello World page returned
  - Script follows the `scripts/` pattern: `set -euo pipefail`, named constants, clear pass/fail output
- **D-05:** Human runs `scripts/verify-dns.sh` after DNS propagation (allow up to 1 hour). DNS-03 is satisfied when this script passes.

### Claude's Discretion
- Exact `dig` flags and output parsing in verify-dns.sh (e.g., `+short`, checking for specific IP in output)
- TTL value to set on GoDaddy records (default GoDaddy TTL of 600s is acceptable)
- Whether to include a propagation wait loop or just print "wait up to 1 hour and re-run"

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

No external specs — requirements fully captured in decisions above and REQUIREMENTS.md (DNS-01 through DNS-03).

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `scripts/setup-aws-infra.sh` — reference script pattern: `set -euo pipefail`, named constants, check-then-create, `--profile kiss` on all aws CLI calls
- `scripts/setup-webroot.sh` and `scripts/setup-iptables.sh` — simpler examples of the pattern

### Established Patterns
- All `scripts/` files: idempotent, `set -euo pipefail`, named constants at top
- Phase scripts are committed to the repo

### Integration Points
- Elastic IP: `54.83.192.65` (Phase 8 output — the A record target)
- EC2 instance responds on port 80 at that IP (Phase 9 verified)
- `scripts/verify-dns.sh` is the only new script produced by this phase

</code_context>

<specifics>
## Specific Ideas

- The plan should document the exact GoDaddy UI steps so any developer can reproduce the configuration
- DNS propagation can take up to 1 hour — the plan should include a note on this and instruct the developer to wait before running verify-dns.sh

</specifics>

<deferred>
## Deferred Ideas

- **Document GoDaddy DNS process in docs/ci-cd.md** — User explicitly requested that Phase 12 (`docs/ci-cd.md`) document the manual GoDaddy DNS configuration process (where to click, what records to set) for future reference.

</deferred>

---

*Phase: 10-dns-configuration*
*Context gathered: 2026-03-24*
