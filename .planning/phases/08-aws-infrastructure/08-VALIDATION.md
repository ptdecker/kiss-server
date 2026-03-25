---
phase: 8
slug: aws-infrastructure
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-11
---

# Phase 8 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | None (bash script execution + AWS CLI queries) |
| **Config file** | none |
| **Quick run command** | `aws ec2 describe-instances --filters "Name=tag:Name,Values=kiss-server" --region us-east-1 --query 'Reservations[0].Instances[0].State.Name' --output text` |
| **Full suite command** | `bash scripts/setup-aws-infra.sh` (idempotent re-run) |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run quick run command
- **After every plan wave:** Run `bash scripts/setup-aws-infra.sh`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 8-01-01 | 01 | 1 | INFRA-01 | smoke | `aws ec2 describe-instances --filters "Name=tag:Name,Values=kiss-server" "Name=instance-state-name,Values=running" --region us-east-1 --query 'Reservations[0].Instances[0].InstanceId' --output text` | ❌ W0 | ⬜ pending |
| 8-01-02 | 01 | 1 | INFRA-02 | smoke | `aws ec2 describe-addresses --filters "Name=tag:Name,Values=kiss-server" --region us-east-1 --query 'Addresses[0].PublicIp' --output text` | ❌ W0 | ⬜ pending |
| 8-01-03 | 01 | 1 | INFRA-03 | smoke | `aws ec2 describe-security-groups --filters "Name=group-name,Values=kiss-server-sg" --region us-east-1 --query 'SecurityGroups[0].IpPermissions'` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `scripts/setup-aws-infra.sh` — idempotent provisioning script (primary deliverable)

*No test framework needed — all verification is via AWS CLI describe commands and live SSH test.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| SSH accessible from developer's IP | INFRA-01 | Requires live SSH connection to EC2 | `ssh -o ConnectTimeout=10 ec2-user@$EIP 'echo OK'` |
| EIP survives stop/start cycle | INFRA-02 | Requires stop + start cycle to verify | Stop instance, start instance, confirm EIP unchanged |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
