---
phase: 08-aws-infrastructure
plan: 02
subsystem: infra
tags: [aws, ec2, elastic-ip, security-group, ssh, amazon-linux-2023]

# Dependency graph
requires:
  - phase: 08-01
    provides: AWS CLI v2 installed, kiss profile configured, default VPC vpc-0af357914ff0ad825
provides:
  - EC2 t3.micro instance i-0394a6d927c0d9b33 running Amazon Linux 2023 x86_64 in us-east-1
  - Elastic IP 54.83.192.65 allocated and associated with instance (allocation eipalloc-0deaa59ab7bff907d)
  - Security Group sg-0cf50c46b18fd13f3 (kiss-server-sg) with ports 22 and 80 open to 0.0.0.0/0
  - Key pair kiss-server imported from ~/.ssh/id_ed25519.pub
  - scripts/setup-aws-infra.sh idempotent provisioning script committed to repo
  - SSH access verified: ec2-user@54.83.192.65 using ~/.ssh/id_ed25519
affects: [09-ec2-setup, 10-dns, 11-cd-pipeline, 12-docs]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Idempotent EC2 provisioning via bash check-then-create pattern with AWS tag-based existence checks"
    - "AMI ID always looked up via SSM parameter (never hardcoded)"

key-files:
  created: [scripts/setup-aws-infra.sh]
  modified: []

key-decisions:
  - "Elastic IP 54.83.192.65 is stable across instance stop/start — all downstream phases use this address"
  - "AMI ami-02dfbd4ff395f2a1b (Amazon Linux 2023 x86_64 kernel-default) was the latest at provisioning time; script re-looks up via SSM on each run"
  - "Security Group port 22 open to 0.0.0.0/0: key auth is the only gate; IP restriction deferred due to dual-location dev + GitHub Actions use case"

patterns-established:
  - "setup-aws-infra.sh: set -euo pipefail, named constants at top, check-then-create idempotency, --profile kiss --region us-east-1 on every call"

requirements-completed: [INFRA-01, INFRA-02, INFRA-03]

# Metrics
duration: 20min
completed: 2026-03-24
---

# Phase 8 Plan 02: EC2 Provisioning Summary

**Idempotent bash script provisions t3.micro instance (Amazon Linux 2023 x86_64), allocates Elastic IP 54.83.192.65, and configures Security Group — SSH access to ec2-user@54.83.192.65 confirmed**

## Performance

- **Duration:** ~20 min
- **Started:** 2026-03-24
- **Completed:** 2026-03-24
- **Tasks:** 3 (2 auto + 1 human-verify checkpoint)
- **Files modified:** 1 (scripts/setup-aws-infra.sh created)

## Accomplishments

- `scripts/setup-aws-infra.sh` written following project pattern: `set -euo pipefail`, named constants, idempotent check-then-create for all 5 resource steps
- Script executed successfully against AWS account 859953692821 — all resources provisioned on first run
- SSH to `ec2-user@54.83.192.65` returned "SSH OK" and confirmed x86_64 architecture
- Idempotency re-run confirmed: all steps printed "already exists, skipping" and same Elastic IP returned

## Task Commits

Each task was committed atomically:

1. **Task 1: Write scripts/setup-aws-infra.sh** - `3c9b687` (feat)
2. **Task 2: Run setup-aws-infra.sh and provision infrastructure** - `87ba71a` (chore)
3. **Task 3: Verify SSH access to EC2 instance** - human-verified checkpoint (no code commit; SSH OK confirmed by user)

**Plan metadata:** committed with SUMMARY.md

## Files Created/Modified

- `scripts/setup-aws-infra.sh` - Idempotent EC2 provisioning script: key pair import, Security Group + rules, AMI lookup via SSM, EC2 run-instances, Elastic IP allocation + association

## Decisions Made

- AMI ID is never hardcoded — always fetched via SSM parameter `/aws/service/ami-amazon-linux-latest/al2023-ami-kernel-default-x86_64` so re-runs always use the latest Amazon Linux 2023 image
- Port 22 open to `0.0.0.0/0`: key auth is the sole gate; restricting CIDRs would require managing home IP + work IP + GitHub Actions IP ranges which is more operational friction than security benefit for this project
- `--credit-specification CpuCredits=standard` added to prevent unexpected burst charges on t3.micro

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - all provisioning was automated via `scripts/setup-aws-infra.sh`. SSH verification was the only human step (checkpoint gate by design).

## Infrastructure Notes

(For Phase 12 docs/ci-cd.md reference)

| Resource | Value |
|----------|-------|
| AWS Account ID | 859953692821 |
| AWS CLI Profile | kiss |
| Region | us-east-1 |
| Default VPC | vpc-0af357914ff0ad825 (172.31.0.0/16) |
| Key pair name | kiss-server |
| Instance ID | i-0394a6d927c0d9b33 |
| Instance type | t3.micro |
| AMI ID | ami-02dfbd4ff395f2a1b (Amazon Linux 2023 x86_64 kernel-default) |
| Elastic IP | 54.83.192.65 |
| EIP Allocation ID | eipalloc-0deaa59ab7bff907d |
| Security Group name | kiss-server-sg |
| Security Group ID | sg-0cf50c46b18fd13f3 |
| SSH user | ec2-user |
| SSH key | ~/.ssh/id_ed25519 |
| Ports open inbound | 22 (SSH, 0.0.0.0/0), 80 (HTTP, 0.0.0.0/0) |

## Next Phase Readiness

- Phase 9 (EC2 service setup): SSH access to `ec2-user@54.83.192.65` confirmed — ready to install kiss-server binary and configure systemd service
- Phase 10 (DNS): Elastic IP `54.83.192.65` is stable — ready to point GoDaddy A record at this address
- Phase 11 (CD pipeline): Key pair `kiss-server` matches `~/.ssh/id_ed25519` — ready to configure GitHub Actions SSH secret
- Phase 12 (docs): All infrastructure values captured in Infrastructure Notes table above

No blockers.

---
*Phase: 08-aws-infrastructure*
*Completed: 2026-03-24*

## Self-Check: PASSED

- FOUND: `.planning/phases/08-aws-infrastructure/08-02-SUMMARY.md`
- FOUND: `scripts/setup-aws-infra.sh`
- FOUND: commit `3c9b687` (Task 1: write setup script)
- FOUND: commit `87ba71a` (Task 2: provision infrastructure)
