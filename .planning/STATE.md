---
gsd_state_version: 1.0
milestone: v1.1
milestone_name: Ops & Deployment
status: Ready to execute
stopped_at: Completed 12.1-01-PLAN.md
last_updated: "2026-03-25T17:09:09.948Z"
progress:
  total_phases: 9
  completed_phases: 8
  total_plans: 19
  completed_plans: 18
---

# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-03-10)

**Core value:** A client can request any static file by path and receive a correct, RFC-compliant HTTP/1.1 response — without crashing, leaking filesystem paths, or serving the wrong content type.
**Current focus:** Phase 12.1 — clean-up-in-aisle-9

## Current Position

Phase: 12.1 (clean-up-in-aisle-9) — EXECUTING
Plan: 4 of 4

## Performance Metrics

**Velocity (v1.0 reference):**

- Total plans completed: 16
- Average duration: ~5 min
- Total execution time: ~1.3 hours

**By Phase (v1.0):**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 01-foundation-fixes | 2/2 | ~9min | ~5min |
| 02-response-and-http-compliance | 3/3 | ~18min | ~6min |
| 03-handler-context-and-router | 3/3 | ~7min | ~2min |
| 04-url-path-safety | 1/1 | ~5min | ~5min |
| 05-static-file-serving | 5/5 | ~20min | ~4min |
| 05.1-address-tech-debt | 2/2 | ~8min | ~4min |

**Recent Trend:** Stable

*Updated after each plan completion*
| Phase 07-branch-protection P01 | 3 | 3 tasks | 1 files |
| Phase 08-aws-infrastructure P01 | 10 | 3 tasks | 0 files |
| Phase 08-aws-infrastructure P02 | 20 | 3 tasks | 1 files |
| Phase 09-ec2-service-setup P01 | 2 | 1 tasks | 2 files |
| Phase 09-ec2-service-setup P02 | 2 | 3 tasks | 3 files |
| Phase 09-ec2-service-setup P03 | 15 | 3 tasks | 1 files |
| Phase 10 P01 | 525582 | 1 tasks | 1 files |
| Phase 10.1-rename-cargo-package-and-directory P01 | 2 | 2 tasks | 6 files |
| Phase 11 P01 | 2 | 2 tasks | 2 files |
| Phase 11 P02 | 13 | 3 tasks | 5 files |
| Phase 12 P01 | 2 | 2 tasks | 2 files |
| Phase 12 P02 | 2 | 2 tasks | 2 files |
| Phase 12.1-clean-up-in-aisle-9 P02 | 4 | 2 tasks | 1 files |
| Phase 12.1-clean-up-in-aisle-9 P03 | 3 | 3 tasks | 3 files |
| Phase 12.1-clean-up-in-aisle-9 P01 | 4 | 2 tasks | 4 files |

## Accumulated Context

### Roadmap Evolution

- Phase 10.1 inserted after Phase 10: rename cargo package and directory (URGENT)
- Phase 12.1 inserted after Phase 12: clean up in aisle 9

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work:

- Roadmap: Phase 8 (INFRA) has no dependency on Phase 7 (BRANCH) — can begin immediately after Phase 6 CI is green; sequential ordering is for clarity only
- Roadmap: Branch protection must come after first CI run (job name must appear in GitHub's check registry before it can be selected as required check)
- Roadmap: Use t3.micro x86_64 (not t4g arm64) — CI runner is x86_64; architecture mismatch causes silent green-CI / broken-prod failure
- Roadmap: CD health check is non-negotiable — systemctl restart exits 0 even on immediate crash; must follow with systemctl is-active check
- Roadmap: SCP to /tmp/ then atomic mv — never SCP over a running binary (SIGBUS risk)
- [Phase 07-branch-protection]: GitHub Rulesets update endpoint uses PUT not PATCH (PATCH returns 404)
- [Phase 07-branch-protection]: RepositoryRole actor_id 5 bypass_mode always preserves solo developer emergency push access to main
- [Phase 08-aws-infrastructure]: Default VPC in us-east-1 was deleted — recreated via aws ec2 create-default-vpc; ID is vpc-0af357914ff0ad825
- [Phase 08-aws-infrastructure]: AWS profile kiss configured for account 859953692821 in us-east-1; all aws CLI commands in this project use --profile kiss
- [Phase 08-aws-infrastructure]: Elastic IP 54.83.192.65 allocated (eipalloc-0deaa59ab7bff907d) and associated with instance i-0394a6d927c0d9b33 — all downstream phases use this stable address
- [Phase 08-aws-infrastructure]: Security Group sg-0cf50c46b18fd13f3 (kiss-server-sg): port 22 open to 0.0.0.0/0, key auth is sole gate; port 80 open to 0.0.0.0/0
- [Phase 09-ec2-service-setup]: parse_root() wrapper removed — main() calls parse_root_from(&args) directly to share args with parse_port_from()
- [Phase 09-ec2-service-setup]: log::debug and log::warn imports moved to server/mod.rs where macros are used; main.rs retains only log::info
- [Phase 09-ec2-service-setup]: install-kiss-server.sh: 512MB swap file created first to prevent OOM during cargo build --release on t3.micro
- [Phase 09-ec2-service-setup]: iptables INPUT ACCEPT for port 8080 required alongside port 80 — redirected traffic traverses INPUT chain on Amazon Linux 2023
- [Phase 09-ec2-service-setup]: gcc (dnf install -y gcc) required before cargo build --release on fresh Amazon Linux 2023 — C linker absent on minimal image
- [Phase 09-ec2-service-setup]: kiss-server deployment: SCP scripts to /tmp/, execute via SSH; script order is install-kiss-server.sh -> setup-webroot.sh -> setup-iptables.sh
- [Phase 10]: verify-dns.sh uses tail -1 on dig output for www CNAME resolution to handle multi-line output; wraps curl|grep in if/else to avoid set -e aborting on grep non-match
- [Phase 10.1]: Option A EC2 migration: update script variables only; /opt/ptodd becomes safe orphan on next deploy run
- [Phase 10.1]: Domain strings ptodd.org intentionally left unchanged in README, main.rs, and WEBROOT variable
- [Phase 11]: Used softprops/action-gh-release@v2 for atomic tag+asset creation in CD pipeline
- [Phase 11]: Static EC2_KNOWN_HOSTS secret over runtime ssh-keyscan to prevent TOFU risk
- [Phase 11]: No required_status_checks on prod ruleset — prod is deploy target, CI gates belong on main
- [Phase 11]: deploy/{sha} tag format distinguishes CD releases from future semver releases
- [Phase 11]: Two PRs required to get all commits onto main: first PR missed 10 local-only commits; second PR resolved after rebasing
- [Phase 11]: Prod deploy command: git push origin origin/main:prod (not from local branch)
- [Phase 12]: Include all 14 key decisions from PROJECT.md in docs/design.md — all are legitimate design decisions explaining the codebase to a new reader
- [Phase 12]: docs/ directory created to house architecture documentation separate from README
- [Phase 12]: Placed CI badge on line 1 before title heading per D-01 (standard OSS convention)
- [Phase 12]: docs/ci-cd.md references scripts/README.md for script details instead of inlining content per D-05
- [Phase 12.1-02]: test.js was untracked (shown as ?? in git status), so plain rm was used instead of git rm
- [Phase 12.1-02]: .planning/ untracked via git rm -r --cached; files remain intact on disk; .gitignore prevents re-tracking
- [Phase 12.1-clean-up-in-aisle-9]: actions/checkout@v4.2.2 and softprops/action-gh-release@v2.2.1 pinned for Node.js 24 compatibility
- [Phase 12.1-clean-up-in-aisle-9]: CD fails hard if no vX.Y.Z tag on HEAD or no CHANGELOG entry — deployment without semver tag and CHANGELOG entry is prohibited
- [Phase 12.1-clean-up-in-aisle-9]: Retain #[allow(clippy::wrong_self_convention)] on to_imf_fixdate — to_ method with &self returning owned String is correct semantics; only dead_code allow was removed
- [Phase 12.1-clean-up-in-aisle-9]: ENABLE_POWERED_BY = true compile-time const flag pattern for X-Powered-By header injection; uses kiss-serve (not kiss-server) per user spec

### Pending Todos

None.

### Blockers/Concerns

- Phase 9 research flag: Amazon Linux 2023 iptables persistence — confirm exact dnf install iptables-services + service iptables save command sequence on the actual instance before executing
- Phase 11 research flag: appleboy/scp-action@v1 destination path structure — action appends source directory tree to target, so mv path may be /tmp/target/release/kiss-server not /tmp/kiss-server; validate on first deploy
- Phase 8 decision needed: GitHub Actions IP ranges for SSH Security Group rule — options: (a) allow 0.0.0.0/0 on port 22 (key auth still required), or (b) use SSM Session Manager (adds IAM complexity)

## Session Continuity

Last session: 2026-03-25T17:09:09.939Z
Stopped at: Completed 12.1-01-PLAN.md
Resume file: None
