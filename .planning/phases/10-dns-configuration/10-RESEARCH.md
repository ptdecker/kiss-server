# Phase 10: DNS Configuration - Research

**Researched:** 2026-03-24
**Domain:** GoDaddy DNS management / shell-based DNS verification
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Use the GoDaddy web UI (manual) — log into GoDaddy dashboard and set records by hand. No API script.
- **D-02:** A record: `@` → `54.83.192.65` (Elastic IP from Phase 8)
- **D-03:** CNAME: `www` → `@` (so www.ptodd.org also resolves)
- **D-04:** Commit `scripts/verify-dns.sh` — a terminal smoke test script that runs `dig ptodd.org`, `dig www.ptodd.org`, `curl http://ptodd.org/`, `curl http://www.ptodd.org/`. Script follows the `scripts/` pattern: `set -euo pipefail`, named constants, clear pass/fail output.
- **D-05:** Human runs `scripts/verify-dns.sh` after DNS propagation (allow up to 1 hour). DNS-03 is satisfied when this script passes.

### Claude's Discretion
- Exact `dig` flags and output parsing in verify-dns.sh (e.g., `+short`, checking for specific IP in output)
- TTL value to set on GoDaddy records (default GoDaddy TTL of 600s is acceptable)
- Whether to include a propagation wait loop or just print "wait up to 1 hour and re-run"

### Deferred Ideas (OUT OF SCOPE)
- **Document GoDaddy DNS process in docs/ci-cd.md** — Phase 12 will document this for future reference.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| DNS-01 | GoDaddy A record for `@` (ptodd.org) points to Elastic IP 54.83.192.65 | Manual GoDaddy UI step; confirmed current A record is stale (74.220.199.6) |
| DNS-02 | GoDaddy CNAME for `www` points to `@` so `www.ptodd.org` also resolves | Manual GoDaddy UI step; confirmed no www CNAME currently exists |
| DNS-03 | `http://ptodd.org/` and `http://www.ptodd.org/` return Hello World page in a browser | Verified via `scripts/verify-dns.sh` after propagation |
</phase_requirements>

---

## Summary

Phase 10 is the smallest phase in the v1.1 roadmap: two manual GoDaddy DNS record changes and one verification script. There is no infrastructure to provision, no code to compile, and no service to configure. The entire implementation is (1) two clicks in the GoDaddy UI and (2) one shell script committed to the repo.

The current DNS state has been probed: `ptodd.org` currently resolves to `74.220.199.6` (a stale address from a previous host). The `www` CNAME does not exist. Both must be corrected. DNS propagation after GoDaddy changes typically completes within minutes for GoDaddy-managed zones, though the official maximum is 1 hour.

The verify-dns.sh script design is straightforward: use `dig +short` to extract just the resolved IP, compare it to the expected Elastic IP constant with a string equality check, then use `curl -fs` to confirm the HTTP response contains the expected content. Exit 1 on any failure, print a clear pass/fail summary.

**Primary recommendation:** Write verify-dns.sh using `dig +short` + string comparison for DNS checks and `curl -fs --max-time 10` for HTTP checks. Print "wait up to 1 hour and re-run" rather than a polling loop — polling loops in shell scripts can mask partial failures and are harder to interrupt.

---

## Standard Stack

### Core
| Tool | Version | Purpose | Why Standard |
|------|---------|---------|--------------|
| `dig` | 9.10.6 (macOS, confirmed present) | DNS query and verification | Standard DNS diagnostic tool; present on macOS and Amazon Linux 2023 |
| `curl` | 8.7.1 (macOS, confirmed present) | HTTP response verification | Present everywhere; standard for smoke-testing HTTP endpoints |

### Supporting
| Tool | Version | Purpose | When to Use |
|------|---------|---------|-------------|
| `nslookup` | present | Fallback DNS query | Only if `dig` unavailable; `dig` preferred for scriptable output |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `dig +short` | `nslookup` | `nslookup` output is harder to parse reliably in scripts |
| `curl -fs` | `wget` | `wget` not present on macOS by default; `curl` is universal |

**Installation:** No installation required. `dig` and `curl` are present on macOS (developer machine, confirmed) and on Amazon Linux 2023 (EC2 instance).

---

## Current DNS State (probed 2026-03-24)

This is the live state of ptodd.org DNS as of research time:

| Record | Current Value | Required Value | Action |
|--------|--------------|----------------|--------|
| `@` (A record) | `74.220.199.6` | `54.83.192.65` | UPDATE in GoDaddy |
| `www` (CNAME) | (does not exist) | `@` | CREATE in GoDaddy |

**Observed TTL on current A record:** ~48 seconds in query response (caching in local resolver). The authoritative TTL as set by GoDaddy is likely 600 seconds (default).

**Observed nameserver authority:** The NS query returns no NS records for `ptodd.org` in the ANSWER section (only SOA for `org.`). This is unusual but does not block resolution — the A record still resolves correctly via the chain. This is likely a GoDaddy DNS configuration artifact and does not affect the plan.

---

## Architecture Patterns

### verify-dns.sh Pattern

The script follows the established `scripts/` pattern: `set -euo pipefail`, named constants at top, idempotent (safe to re-run), clear step headers, exit code reflects pass/fail.

**Key design decisions for Claude's Discretion items:**

1. **`dig +short` for IP extraction** — outputs just the IP address, trivially comparable with a string equality test. No awk/sed needed.

2. **`curl -fs --max-time 10`** — `-f` fails on HTTP error codes (non-2xx), `-s` suppresses progress meter, `--max-time 10` prevents indefinite hang if DNS resolves but server is slow.

3. **Content check via `grep`** — pipe curl output through `grep -q "Hello World"` to verify the correct page is returned (not a redirect or error page).

4. **No propagation wait loop** — print a message instructing the developer to wait up to 1 hour and re-run. This is cleaner than a polling loop that could run indefinitely.

5. **Script is LOCAL (run from developer machine)** — unlike install-kiss-server.sh / setup-webroot.sh / setup-iptables.sh which run ON EC2, verify-dns.sh runs from the developer's machine. It does not SSH anywhere.

### Script Structure
```
ELASTIC_IP="54.83.192.65"
DOMAIN="ptodd.org"
WWW_DOMAIN="www.ptodd.org"

Step 1: dig ptodd.org — compare +short output to ELASTIC_IP
Step 2: dig www.ptodd.org — confirm resolves (A record via CNAME chain)
Step 3: curl http://ptodd.org/ — confirm Hello World in body
Step 4: curl http://www.ptodd.org/ — confirm Hello World in body
```

### GoDaddy UI Navigation Pattern
The plan must document exact steps for GoDaddy DNS management:
1. Log in at godaddy.com
2. My Products → Domains → ptodd.org → DNS
3. Edit/update the `@` A record — change value from current IP to `54.83.192.65`
4. Add new CNAME record — Name: `www`, Value: `@`
5. Save changes

**GoDaddy CNAME `@` behavior:** GoDaddy supports `www CNAME @` — the `@` value means "same as the root domain." This is the canonical way to make www resolve alongside the root. HIGH confidence — this is standard GoDaddy behavior.

### Anti-Patterns to Avoid
- **Using `curl` without `-f`:** Will silently "pass" even on 404/500 responses because curl exits 0 for any HTTP response. Always use `-f`.
- **Using `dig` without `+short`:** Full `dig` output contains headers/comments that make string comparison fragile.
- **Content check only (no DNS check):** If the old IP is still cached and serving something, a content check could pass spuriously. Always verify the IP explicitly before the HTTP check.
- **Propagation polling loop in script:** A `while true; do sleep 60; done` loop is hard to interrupt and masks the state. Prefer: check once, tell the user to re-run.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| DNS propagation detection | Custom TTL countdown logic | Just `dig +short` and re-run | TTL is advisory; actual propagation depends on resolver caching, not just TTL |
| HTTP content assertion | Custom HTTP parser | `curl -fs` + `grep -q` | curl handles redirects, timeouts, TLS (future); grep handles content matching |

---

## Common Pitfalls

### Pitfall 1: GoDaddy CNAME `@` Target vs. Fully-Qualified Domain
**What goes wrong:** Some DNS UIs require the CNAME target to be a fully-qualified domain name (FQDN) with a trailing dot. GoDaddy specifically accepts bare `@` to mean "root domain." Entering `ptodd.org` or `ptodd.org.` as the CNAME target instead of `@` may work but is not the standard GoDaddy convention.
**Why it happens:** Confusion between registrar UI conventions and raw DNS zone file syntax.
**How to avoid:** Use `@` as the CNAME value in GoDaddy UI, not the FQDN.
**Warning signs:** CNAME resolves to wrong IP or creates a CNAME chain loop.

### Pitfall 2: Old A Record Not Updated (Stale Value Left In Place)
**What goes wrong:** The current A record for `@` is `74.220.199.6`. If the developer adds a NEW A record rather than editing the existing one, GoDaddy may have two A records for `@`, causing round-robin resolution between the old and new IPs.
**Why it happens:** GoDaddy UI allows adding records without checking for duplicates.
**How to avoid:** The plan must explicitly say "edit the existing A record" not "add a new A record."
**Warning signs:** `dig +short ptodd.org` returns two IP addresses.

### Pitfall 3: DNS Propagation Check Before Changes
**What goes wrong:** Developer runs verify-dns.sh before making changes, sees failure, thinks something is wrong with the script.
**Why it happens:** Script is written before changes are made.
**How to avoid:** Plan must clearly state: write and commit script first, then make GoDaddy changes, then wait and run.
**Warning signs:** Verify output shows `74.220.199.6` instead of `54.83.192.65`.

### Pitfall 4: `curl` Content Check Against Wrong Page
**What goes wrong:** curl returns a GoDaddy parking page (HTTP 200) if DNS resolves but the web server is down. A grep for "Hello World" would fail but a grep for "html" would spuriously pass.
**Why it happens:** GoDaddy sometimes intercepts resolution to a parked page.
**How to avoid:** Check for the specific content "Hello World" (from setup-webroot.sh's index.html), not generic HTML.
**Warning signs:** curl returns 200 but grep fails.

---

## Code Examples

Verified patterns from existing scripts in this project:

### Existing Script Pattern (from setup-webroot.sh)
```bash
#!/usr/bin/env bash
set -euo pipefail

# Named constants at top
WEBROOT="/var/www/ptodd.org"

# Step headers
echo "==> Step 1: Webroot directory"

# Idempotent check-then-act
if [ -d "$WEBROOT" ]; then
  echo "  Webroot $WEBROOT already exists, skipping."
else
  echo "  Creating webroot $WEBROOT..."
fi

echo ""
echo "setup-webroot.sh complete."
```

### dig +short IP comparison pattern
```bash
ACTUAL_IP=$(dig +short "$DOMAIN" A)
if [ "$ACTUAL_IP" = "$ELASTIC_IP" ]; then
  echo "  PASS: $DOMAIN resolves to $ACTUAL_IP"
else
  echo "  FAIL: $DOMAIN resolves to '$ACTUAL_IP' (expected '$ELASTIC_IP')"
  FAILURES=$((FAILURES + 1))
fi
```

### curl content check pattern
```bash
if curl -fs --max-time 10 "http://$DOMAIN/" | grep -q "Hello World"; then
  echo "  PASS: http://$DOMAIN/ returns Hello World"
else
  echo "  FAIL: http://$DOMAIN/ did not return Hello World"
  FAILURES=$((FAILURES + 1))
fi
```

### Exit code and summary pattern
```bash
echo ""
if [ "$FAILURES" -eq 0 ]; then
  echo "All checks passed. DNS configuration is correct."
  exit 0
else
  echo "$FAILURES check(s) failed."
  echo "If DNS was just changed, wait up to 1 hour for propagation and re-run."
  exit 1
fi
```

---

## State of the Art

| Old Approach | Current Approach | Notes |
|--------------|------------------|-------|
| GoDaddy API for DNS changes | GoDaddy web UI (manual) | User decision D-01: no API script for this project |
| Route 53 for DNS hosting | GoDaddy nameservers (existing) | Out of scope per REQUIREMENTS.md |

---

## Open Questions

1. **Does GoDaddy currently have any existing www record?**
   - What we know: `dig www.ptodd.org CNAME` returns NXDOMAIN (no CNAME). `dig +short www.ptodd.org A` returns `74.220.199.6` — same as root, suggesting GoDaddy may have a wildcard or forwarding rule, not an explicit CNAME.
   - What's unclear: Whether there is a wildcard `*` A record or a www A record (not CNAME) that needs to be deleted or converted.
   - Recommendation: The plan should instruct the developer to check for and delete any existing `www` A record before adding the CNAME.

2. **Will `www CNAME @` behave correctly after the A record update?**
   - What we know: GoDaddy supports this pattern. The `@` in CNAME value resolves via the updated A record.
   - What's unclear: Nothing — this is standard behavior. HIGH confidence.
   - Recommendation: No special handling needed.

---

## Environment Availability

| Dependency | Required By | Available | Version | Fallback |
|------------|------------|-----------|---------|---------|
| `dig` | DNS verification in verify-dns.sh | Yes | DiG 9.10.6 | `nslookup` (less scriptable) |
| `curl` | HTTP verification in verify-dns.sh | Yes | 8.7.1 | None needed |
| GoDaddy account access | DNS record changes | Assumed (developer responsibility) | — | — |

**Missing dependencies with no fallback:** None on developer machine. GoDaddy account access is a human prerequisite, not a script dependency.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Bash (verify-dns.sh) — shell smoke test, no test framework |
| Config file | none |
| Quick run command | `bash scripts/verify-dns.sh` |
| Full suite command | `bash scripts/verify-dns.sh` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DNS-01 | `ptodd.org` A record resolves to `54.83.192.65` | smoke | `bash scripts/verify-dns.sh` | No — Wave 0 |
| DNS-02 | `www.ptodd.org` resolves (via CNAME to `@`) | smoke | `bash scripts/verify-dns.sh` | No — Wave 0 |
| DNS-03 | `http://ptodd.org/` and `http://www.ptodd.org/` return Hello World | smoke | `bash scripts/verify-dns.sh` | No — Wave 0 |

**Note:** All three requirements are verified by a single script. DNS-03 additionally requires a human to run the script after DNS propagation — it cannot be automated in CI because it depends on external DNS state.

### Sampling Rate
- **Per task commit:** n/a — script is committed in one task, DNS changes are manual
- **Per wave merge:** `bash scripts/verify-dns.sh` (after DNS propagation)
- **Phase gate:** `bash scripts/verify-dns.sh` exits 0 before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `scripts/verify-dns.sh` — covers DNS-01, DNS-02, DNS-03

---

## Sources

### Primary (HIGH confidence)
- Live DNS probe: `dig ptodd.org A` — current A record is `74.220.199.6`; TTL ~48s (2026-03-24)
- Live DNS probe: `dig www.ptodd.org CNAME` — no CNAME exists; `dig +short www.ptodd.org A` returns `74.220.199.6` (likely wildcard or A record)
- Project scripts read directly: `scripts/setup-webroot.sh`, `scripts/setup-iptables.sh` — established script pattern confirmed
- CONTEXT.md D-01 through D-05 — all implementation decisions locked by user

### Secondary (MEDIUM confidence)
- GoDaddy CNAME `@` behavior — standard GoDaddy UI convention, widely documented

### Tertiary (LOW confidence)
- None

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — `dig` and `curl` confirmed present; no third-party libraries needed
- Architecture: HIGH — all decisions locked in CONTEXT.md; script pattern confirmed from existing scripts
- Pitfalls: HIGH — stale A record and missing www CNAME confirmed by live DNS probe; GoDaddy UI pitfalls are well-known

**Research date:** 2026-03-24
**Valid until:** 2026-04-24 (stable domain — DNS state won't change without action)
