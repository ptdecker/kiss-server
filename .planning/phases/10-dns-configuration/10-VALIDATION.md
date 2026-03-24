---
phase: 10
slug: dns-configuration
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-24
---

# Phase 10 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Bash (verify-dns.sh) — shell smoke test, no test framework |
| **Config file** | none |
| **Quick run command** | `bash scripts/verify-dns.sh` |
| **Full suite command** | `bash scripts/verify-dns.sh` |
| **Estimated runtime** | ~5 seconds (network-dependent) |

---

## Sampling Rate

- **After every task commit:** n/a — script is committed in one task; DNS changes are manual
- **After every plan wave:** `bash scripts/verify-dns.sh` (after DNS propagation completes)
- **Before `/gsd:verify-work`:** `bash scripts/verify-dns.sh` must exit 0
- **Max feedback latency:** ~5 seconds (plus DNS propagation wait — up to 1 hour)

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 10-01-01 | 01 | 1 | DNS-01, DNS-02, DNS-03 | smoke | `bash scripts/verify-dns.sh` | ❌ W0 | ⬜ pending |
| 10-02-01 | 02 | 2 | DNS-01, DNS-02, DNS-03 | smoke (manual trigger) | `bash scripts/verify-dns.sh` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `scripts/verify-dns.sh` — smoke test for DNS-01, DNS-02, DNS-03 (created in Plan 01)

*Note: Wave 0 is satisfied when the script is written and committed. The script cannot pass until DNS propagation completes after the manual GoDaddy steps in Plan 02.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| GoDaddy A record set to 54.83.192.65 | DNS-01 | External DNS provider UI — no CLI automation without API key | Log into GoDaddy dashboard → DNS Management for ptodd.org → edit/create @ A record → set value to 54.83.192.65 |
| GoDaddy CNAME www → @ | DNS-02 | External DNS provider UI | Same DNS Management page → add/edit www CNAME → set value to @ |
| Browser loads Hello World | DNS-03 | Requires DNS propagation + browser | After verify-dns.sh passes: open http://ptodd.org/ and http://www.ptodd.org/ in browser |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 10s (after propagation)
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
