---
phase: 6
slug: ci-pipeline
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-10
---

# Phase 6 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[test]` (cargo test) |
| **Config file** | none — cargo discovers tests automatically |
| **Quick run command** | `cargo test` |
| **Full suite command** | `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test` |
| **Estimated runtime** | ~10 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
- **After every plan wave:** Run full suite command above
- **Before `/gsd:verify-work`:** Full suite must be green + all GitHub Actions verifications completed
- **Max feedback latency:** ~10 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| rust-toolchain.toml | 01 | 1 | CI-05 | smoke | `cat rust-toolchain.toml` | ❌ Wave 0 | ⬜ pending |
| scripts/ci.sh | 01 | 1 | CI-02, CI-03, CI-04 | smoke | `./scripts/ci.sh` | ❌ Wave 0 | ⬜ pending |
| ci.yml workflow | 01 | 1 | CI-01, CI-02, CI-03, CI-04, CI-06 | manual | View GitHub Actions tab | N/A — GitHub-side | ⬜ pending |
| CI-01 trigger | 01 | 1 | CI-01 | manual | Push to main, verify Actions tab | N/A | ⬜ pending |
| CI-02 fmt gate | 01 | 1 | CI-02 | manual | Introduce fmt error, open PR, verify red | N/A | ⬜ pending |
| CI-03 clippy gate | 01 | 1 | CI-03 | manual | Introduce clippy warning, open PR, verify red | N/A | ⬜ pending |
| CI-04 test gate | 01 | 1 | CI-04 | manual | Break a test, open PR, verify red | N/A | ⬜ pending |
| CI-06 cache | 01 | 1 | CI-06 | manual | Second CI run shows "Cache restored" in Actions log | N/A | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `rust-toolchain.toml` — CI-05: toolchain pin file (create in Wave 1, it is the deliverable)
- [ ] `scripts/ci.sh` — CI-02/03/04: local CI script (create in Wave 1, chmod +x required)
- [ ] `.github/workflows/ci.yml` — CI-01/02/03/04/06: workflow file (create in Wave 1, delete rust.yml)

*No test framework gaps — cargo test is built-in and already functional.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Workflow triggers on push/PR to main | CI-01 | GitHub-side behavior, not verifiable locally | Push commit to main or open PR; check Actions tab shows run |
| fmt failure shows red check on PR | CI-02 | Requires GitHub PR + status check API | Introduce `cargo fmt`-fixable error, open PR, verify red CI check |
| clippy warning shows red check on PR | CI-03 | Requires GitHub PR + status check API | Add `let x = 5;` unused variable, open PR, verify red CI check |
| test failure shows red check on PR | CI-04 | Requires GitHub PR + status check API | Change `assert_eq!(1+1, 2)` to `assert_eq!(1+1, 3)`, open PR, verify red |
| Cache restored on second run | CI-06 | Requires two sequential GitHub Actions runs | Trigger CI twice; second run must show "Restored cache" in Swatinem step |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
