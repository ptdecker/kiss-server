---
phase: 9
slug: ec2-service-setup
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-24
---

# Phase 9 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in (`cargo test`) + SSH/curl smoke checks |
| **Config file** | `rust-toolchain.toml` |
| **Quick run command** | `cargo test` |
| **Full suite command** | `cargo test` |
| **Estimated runtime** | ~5 seconds (local unit tests) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test`
- **After every plan wave:** Run `cargo test` + SSH/curl smoke checks against EC2
- **Before `/gsd:verify-work`:** Full suite green + `curl http://54.83.192.65/` returns 200 with Hello World
- **Max feedback latency:** ~5 seconds (unit tests); ~15 seconds (with SSH smoke)

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 9-01-01 | 01 | 0 | DEPLOY-03 | unit | `cargo test parse_port` | ❌ W0 | ⬜ pending |
| 9-02-01 | 02 | 1 | DEPLOY-01 | smoke | `ssh ec2-user@54.83.192.65 'test -x /usr/local/bin/kiss-server && echo OK'` | ❌ W0 | ⬜ pending |
| 9-02-02 | 02 | 1 | DEPLOY-02 | smoke | `ssh ec2-user@54.83.192.65 'systemctl is-active kiss-server'` | ❌ W0 | ⬜ pending |
| 9-02-03 | 02 | 1 | DEPLOY-04 | smoke | `ssh ec2-user@54.83.192.65 'test -d /var/www/ptodd.org && echo OK'` | ❌ W0 | ⬜ pending |
| 9-02-04 | 02 | 1 | DEPLOY-05 | smoke | `curl -s http://54.83.192.65/ \| grep -q "Hello World" && echo OK` | ❌ W0 | ⬜ pending |
| 9-02-05 | 02 | 1 | DEPLOY-03 | smoke | `curl -s -o /dev/null -w "%{http_code}" http://54.83.192.65/` → 200 | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Unit tests for `parse_port_from()` in `src/main.rs` — three test cases following `parse_root_from()` pattern

*SSH smoke checks and curl verification are ad-hoc commands run inline as task verify steps — no additional test files needed.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| systemd service restarts on failure | DEPLOY-02 | Requires killing the process to observe restart | `ssh ec2-user@54.83.192.65 'sudo systemctl kill -s SIGKILL kiss-server && sleep 3 && systemctl is-active kiss-server'` |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
