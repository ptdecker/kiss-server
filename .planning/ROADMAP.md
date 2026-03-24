# Roadmap: ptodd

## Milestones

- ✅ **v1.0 MVP** — Phases 1–5.1 (shipped 2026-03-10)
- 🚧 **v1.1 Ops & Deployment** — Phases 6–12 (in progress)

## Phases

<details>
<summary>✅ v1.0 MVP (Phases 1–5.1) — SHIPPED 2026-03-10</summary>

- [x] Phase 1: Foundation Fixes (2/2 plans) — completed 2026-03-01
- [x] Phase 2: Response and HTTP Compliance (3/3 plans) — completed 2026-03-02
- [x] Phase 3: Handler, Context, and Router (3/3 plans) — completed 2026-03-02
- [x] Phase 4: URL Path Safety (1/1 plan) — completed 2026-03-02
- [x] Phase 5: Static File Serving (5/5 plans) — completed 2026-03-10
- [x] Phase 5.1: Address Tech Debt (2/2 plans) — completed 2026-03-10

See `.planning/milestones/v1.0-ROADMAP.md` for full phase details.

</details>

### 🚧 v1.1 Ops & Deployment (In Progress)

**Milestone Goal:** kiss-server runs live at ptodd.org — automated build verification, branch protection, AWS deployment, domain routing, and continuous deployment from the prod branch.

- [x] **Phase 6: CI Pipeline** — GitHub Actions workflow that lints, builds, and tests on every push and PR (completed 2026-03-10)
- [x] **Phase 7: Branch Protection** — main branch requires PR and passing CI before merge (completed 2026-03-11)
- [ ] **Phase 8: AWS Infrastructure** — EC2 instance with Elastic IP and Security Group
- [ ] **Phase 9: EC2 Service Setup** — kiss-server running as a systemd service with Hello World site
- [ ] **Phase 10: DNS Configuration** — ptodd.org and www.ptodd.org routed to EC2 via GoDaddy
- [ ] **Phase 11: CD Pipeline** — prod branch push triggers automated deploy to EC2 and GitHub Release
- [ ] **Phase 12: Badge, Docs, README** — build badge, CI/CD documentation, and updated README

## Phase Details

### Phase 6: CI Pipeline
**Goal**: Every push and pull request is automatically verified — formatting, lint, and tests must pass before code can merge
**Depends on**: Nothing (first v1.1 phase)
**Requirements**: CI-01, CI-02, CI-03, CI-04, CI-05, CI-06
**Success Criteria** (what must be TRUE):
  1. Pushing to main or a milestone branch triggers a CI run visible in the GitHub Actions tab
  2. Opening a PR to main triggers a CI run and posts a status check on the PR
  3. A PR with a formatting error, clippy warning, or failing test shows a red status check and cannot be merged
  4. The Rust toolchain version is pinned in rust-toolchain.toml so CI uses a fixed version regardless of upstream releases
  5. CI completes faster on repeated runs because Cargo registry and build artifacts are restored from cache
**Plans**: 2 plans
Plans:
- [ ] 06-01-PLAN.md — Create rust-toolchain.toml, scripts/ci.sh, and .github/workflows/ci.yml; delete rust.yml
- [ ] 06-02-PLAN.md — Push to GitHub, verify first CI run green, verify failure gates and cache

### Phase 7: Branch Protection
**Goal**: Direct pushes to main are blocked — all changes must arrive via a PR with a passing CI check
**Depends on**: Phase 6 (CI must have run at least once for the check name to appear in GitHub's registry)
**Requirements**: BRANCH-01, BRANCH-02
**Success Criteria** (what must be TRUE):
  1. Attempting to push a commit directly to main is rejected by GitHub
  2. A PR to main cannot be merged until the CI status check reports passing
  3. The branch protection rule does not lock out the solo developer (admin bypass is not disabled)
**Plans**: 1 plan
Plans:
- [ ] 07-01-PLAN.md — Set merge strategy, create idempotent setup-branch-protection.sh, apply ruleset live, verify direct push blocked and PR merge gated on CI

### Phase 8: AWS Infrastructure
**Goal**: A stable, accessible EC2 instance exists with a permanent public IP and correct network access rules
**Depends on**: Nothing (can run in parallel with Phase 7; must complete before Phase 9)
**Requirements**: INFRA-01, INFRA-02, INFRA-03
**Success Criteria** (what must be TRUE):
  1. An EC2 t3.micro instance (Amazon Linux 2023, x86_64) is running and reachable via SSH from the developer's IP
  2. An Elastic IP is allocated and associated with the instance — the IP does not change across stop/start cycles
  3. Port 80 accepts connections from any IP; port 22 accepts connections only from authorized IPs
**Plans**: 2 plans
Plans:
- [x] 08-01-PLAN.md — Install AWS CLI v2, configure kiss profile credentials, verify default VPC exists
- [x] 08-02-PLAN.md — Write and run scripts/setup-aws-infra.sh, provision EC2/SG/EIP, verify SSH access

### Phase 9: EC2 Service Setup
**Goal**: kiss-server runs as a managed systemd service on EC2 and serves a Hello World page on port 80
**Depends on**: Phase 8 (EC2 instance must exist)
**Requirements**: DEPLOY-01, DEPLOY-02, DEPLOY-03, DEPLOY-04, DEPLOY-05
**Success Criteria** (what must be TRUE):
  1. The kiss-server binary is installed at /usr/local/bin/kiss-server on the EC2 instance
  2. The kiss-server systemd service starts automatically on boot and restarts on failure without manual intervention
  3. curl http://[elastic-ip]/ returns 200 with the Hello World page content
  4. The service runs as a non-root user — port 80 traffic reaches it via iptables redirect from 80 to 8080
  5. /var/www/ptodd.org/index.html exists and is the file served at the root path
**Plans**: 3 plans
Plans:
- [ ] 09-01-PLAN.md — Add --port flag and 0.0.0.0 bind to src/main.rs (TDD, with unit tests)
- [ ] 09-02-PLAN.md — Write install-kiss-server.sh, setup-webroot.sh, and setup-iptables.sh
- [ ] 09-03-PLAN.md — Execute scripts on EC2 via SSH, smoke-test all DEPLOY requirements, human verify

### Phase 10: DNS Configuration
**Goal**: ptodd.org and www.ptodd.org resolve to the EC2 instance and return the Hello World page in a browser
**Depends on**: Phase 8 (Elastic IP must exist as the A record target), Phase 9 (EC2 must be responding on port 80)
**Requirements**: DNS-01, DNS-02, DNS-03
**Success Criteria** (what must be TRUE):
  1. A GoDaddy A record for @ (ptodd.org) points to the Elastic IP
  2. A GoDaddy CNAME for www points to @ so www.ptodd.org also resolves
  3. Opening http://ptodd.org/ and http://www.ptodd.org/ in a browser returns the Hello World page
**Plans**: TBD

### Phase 11: CD Pipeline
**Goal**: Pushing to the prod branch automatically builds, deploys, and releases kiss-server to EC2 with a verified health check
**Depends on**: Phase 6 (CI workflow and cache infrastructure), Phase 8 (EC2 SSH target), Phase 9 (systemd service exists to stop/start)
**Requirements**: CD-01, CD-02, CD-03, CD-04, CD-05
**Success Criteria** (what must be TRUE):
  1. Pushing a commit to the prod branch triggers the CD workflow in GitHub Actions
  2. The CD pipeline builds a release binary using cargo build --release on the CI runner
  3. The binary is deployed atomically — SCP to /tmp/, service stopped, binary replaced via mv, service started
  4. The pipeline fails and surfaces an error if the service is not active after deployment (systemctl is-active check)
  5. A GitHub Release tagged with the prod commit SHA is created with the compiled binary attached as an asset
**Plans**: TBD

### Phase 12: Badge, Docs, README
**Goal**: The repository communicates its current state — CI status is visible at a glance, and the pipeline is documented for future reference
**Depends on**: Phase 6 (CI workflow file must exist for the badge URL to resolve)
**Requirements**: DOCS-01, DOCS-02, DOCS-03
**Success Criteria** (what must be TRUE):
  1. The README.md header displays a GitHub Actions build status badge that reflects the current CI pass/fail state
  2. docs/ci-cd.md explains how to trigger CI, how to deploy via the prod branch, how to check EC2 service health, and how to set up the pipeline from scratch
  3. README.md accurately describes the project, how to build and run locally, and how deployment works
**Plans**: TBD

## Progress

**Execution Order:** Phases execute in numeric order: 6 → 7 → 8 → 9 → 10 → 11 → 12

Note: Phase 8 has no dependency on Phase 7 and can begin once Phase 6 CI is green. The ordering above places them sequentially for clarity.

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Foundation Fixes | v1.0 | 2/2 | Complete | 2026-03-01 |
| 2. Response and HTTP Compliance | v1.0 | 3/3 | Complete | 2026-03-02 |
| 3. Handler, Context, and Router | v1.0 | 3/3 | Complete | 2026-03-02 |
| 4. URL Path Safety | v1.0 | 1/1 | Complete | 2026-03-02 |
| 5. Static File Serving | v1.0 | 5/5 | Complete | 2026-03-10 |
| 5.1. Address Tech Debt | v1.0 | 2/2 | Complete | 2026-03-10 |
| 6. CI Pipeline | 2/2 | Complete   | 2026-03-10 | - |
| 7. Branch Protection | 1/1 | Complete   | 2026-03-11 | - |
| 8. AWS Infrastructure | v1.1 | 1/2 | In Progress|  |
| 9. EC2 Service Setup | v1.1 | 0/? | Not started | - |
| 10. DNS Configuration | v1.1 | 0/? | Not started | - |
| 11. CD Pipeline | v1.1 | 0/? | Not started | - |
| 12. Badge, Docs, README | v1.1 | 0/? | Not started | - |
