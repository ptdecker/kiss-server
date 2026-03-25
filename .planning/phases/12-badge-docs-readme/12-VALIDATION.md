---
phase: 12
slug: badge-docs-readme
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-24
---

# Phase 12 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | none — docs-only phase, no test framework |
| **Config file** | none |
| **Quick run command** | `grep -c "badge.svg" README.md && test -f docs/ci-cd.md && test -f docs/design.md && test -f scripts/README.md` |
| **Full suite command** | `grep -c "badge.svg" README.md && test -f docs/ci-cd.md && test -f docs/design.md && test -f scripts/README.md` |
| **Estimated runtime** | ~1 second |

---

## Sampling Rate

- **After every task commit:** Run `grep -c "badge.svg" README.md && test -f docs/ci-cd.md && test -f docs/design.md && test -f scripts/README.md`
- **After every plan wave:** Run full file-existence + content grep suite
- **Before `/gsd:verify-work`:** All files exist and contain required sections
- **Max feedback latency:** 5 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 12-01-01 | 01 | 1 | DOCS-01 | grep | `grep "badge.svg" README.md` | ❌ W0 | ⬜ pending |
| 12-01-02 | 01 | 1 | DOCS-03 | grep | `grep "cargo build" README.md` | ❌ W0 | ⬜ pending |
| 12-02-01 | 02 | 2 | DOCS-02 | file | `test -f docs/ci-cd.md` | ❌ W0 | ⬜ pending |
| 12-03-01 | 03 | 3 | DOCS-03 | grep | `grep "design.md" docs/design.md 2>/dev/null \|\| test -f docs/design.md` | ❌ W0 | ⬜ pending |
| 12-04-01 | 04 | 4 | DOCS-03 | file | `test -f scripts/README.md` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

*Existing infrastructure covers all phase requirements — this is a docs-only phase. No test stubs needed.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Badge renders correctly on GitHub | DOCS-01 | Requires live GitHub rendering | Open https://github.com/ptdecker/kiss-server and verify badge displays with pass/fail state |
| ci-cd.md coverage complete | DOCS-02 | Section completeness is subjective | Read docs/ci-cd.md and verify all 4 sections present: CI triggers, deploy via prod, EC2 health check, setup from scratch |
| README.md sections complete | DOCS-03 | Section completeness is subjective | Read README.md and verify all 6 sections from D-07 are present and accurate |
| Architecture accuracy in design.md | DOCS-03 | Content accuracy requires human judgment | Read docs/design.md and verify Handler/Context/Router pattern, thread pool model, error handling are described |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 5s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
