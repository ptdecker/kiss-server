---
phase: 7
slug: branch-protection
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-11
---

# Phase 7 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Manual verification via `gh` CLI + `git` |
| **Config file** | none — no test framework; all via shell commands |
| **Quick run command** | `gh api /repos/ptdecker/kiss-server/rulesets` |
| **Full suite command** | `git push origin HEAD:main` (expect rejection) |
| **Estimated runtime** | ~10 seconds |

---

## Sampling Rate

- **After every task commit:** Run `gh api /repos/ptdecker/kiss-server/rulesets`
- **After every plan wave:** Run `git push origin HEAD:main` (verify non-zero exit)
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 10 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 07-01-01 | 01 | 1 | BRANCH-01 | smoke | `gh repo edit ptdecker/kiss-server --enable-rebase-merge=false --enable-squash-merge=false && gh api /repos/ptdecker/kiss-server \| python3 -c "import sys,json; d=json.load(sys.stdin); print('rebase:', d['allow_rebase_merge'], 'squash:', d['allow_squash_merge'])"` | ✅ | ⬜ pending |
| 07-01-02 | 01 | 1 | BRANCH-01, BRANCH-02 | smoke | `gh api /repos/ptdecker/kiss-server/rulesets \| python3 -c "import sys,json; r=json.load(sys.stdin); print(len(r), 'rulesets')"` | ✅ Wave 0 | ⬜ pending |
| 07-01-03 | 01 | 1 | BRANCH-01 | smoke | `git commit --allow-empty -m "test: verify branch protection" && git push origin HEAD:main; echo "exit: $?"; git reset --hard HEAD~1` | ✅ Wave 0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] No test framework install needed — `gh` CLI and `git` are already available

*Existing infrastructure covers all phase requirements. Wave 0 is the verification script itself (task 07-01-03).*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| PR cannot merge without passing CI | BRANCH-02 | Requires GitHub UI interaction (merge button state) | Open a PR targeting main, verify merge button shows "Waiting for status checks" or is disabled until `ci` job completes |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
