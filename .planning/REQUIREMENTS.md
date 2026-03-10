# Requirements: kiss-server

**Defined:** 2026-03-10
**Core Value:** A client can request any static file by path and receive a correct, RFC-compliant HTTP/1.1 response — without crashing, leaking filesystem paths, or serving the wrong content type.

## v1.1 Requirements

### CI — Continuous Integration

- [ ] **CI-01**: Developer can see CI run on every push to `main` and milestone branches and on every PR targeting `main`
- [ ] **CI-02**: CI fails (and blocks PR merge) if `cargo fmt -- --check` reports formatting issues
- [ ] **CI-03**: CI fails (and blocks PR merge) if `cargo clippy -- -D warnings` reports any lint warning
- [ ] **CI-04**: CI fails (and blocks PR merge) if any `cargo test` test fails
- [ ] **CI-05**: Rust toolchain version is pinned in `rust-toolchain.toml` to prevent surprise lint failures from toolchain updates
- [ ] **CI-06**: Cargo registry and build artifacts are cached between CI runs to reduce build time

### BRANCH — Branch Protection

- [ ] **BRANCH-01**: Developer cannot push directly to `main` — all changes must go through a PR
- [ ] **BRANCH-02**: A PR cannot be merged to `main` unless the CI status check has passed

### INFRA — AWS Infrastructure

- [ ] **INFRA-01**: An EC2 t3.micro instance (Amazon Linux 2023, x86_64) exists and is accessible via SSH
- [ ] **INFRA-02**: An Elastic IP is allocated and associated with the EC2 instance (stable, survives stop/start)
- [ ] **INFRA-03**: Security Group allows port 80 inbound from `0.0.0.0/0` and port 22 inbound from authorized IPs only

### DEPLOY — EC2 Service Setup

- [ ] **DEPLOY-01**: kiss-server binary is installed at `/usr/local/bin/kiss-server` on EC2
- [ ] **DEPLOY-02**: kiss-server runs as a systemd service that starts on boot and restarts on failure
- [ ] **DEPLOY-03**: kiss-server listens on port 8080; iptables redirects port 80 → 8080 so non-root process can serve HTTP
- [ ] **DEPLOY-04**: `/var/www/ptodd.org/` directory exists on EC2 as the static file root
- [ ] **DEPLOY-05**: A "Hello World" `index.html` is deployed at `/var/www/ptodd.org/index.html` and served correctly

### DNS — Domain Routing

- [ ] **DNS-01**: GoDaddy A record for `@` (ptodd.org) points to the Elastic IP
- [ ] **DNS-02**: GoDaddy CNAME for `www` points to `@` so `www.ptodd.org` also resolves
- [ ] **DNS-03**: `http://ptodd.org/` and `http://www.ptodd.org/` return the Hello World page in a browser

### CD — Continuous Deployment

- [ ] **CD-01**: Pushing to the `prod` branch automatically triggers a deployment to EC2
- [ ] **CD-02**: CD pipeline builds a release binary (`cargo build --release`) on the CI runner
- [ ] **CD-03**: CD pipeline atomically replaces the binary on EC2 (SCP to temp → stop → mv → start)
- [ ] **CD-04**: CD pipeline verifies the service is running after deploy (`systemctl is-active`) and fails the pipeline if it is not
- [ ] **CD-05**: CD pipeline creates a GitHub Release tagged with the prod commit SHA and attaches the compiled binary as a release asset

### DOCS — Badge, Documentation, README

- [ ] **DOCS-01**: README.md displays a GitHub Actions build status badge showing CI pass/fail
- [ ] **DOCS-02**: `docs/ci-cd.md` documents how the CI and CD pipelines work and how to use them
- [ ] **DOCS-03**: README.md is updated to reflect the project's current state, how to build/run, and how deployment works

## Future Requirements

*(none identified — all goals scoped to v1.1)*

## Out of Scope

| Feature | Reason |
|---------|--------|
| TLS/HTTPS (Let's Encrypt) | Adds certbot complexity; HTTP sufficient for learning project at this stage |
| Zero-downtime deployment | Overkill for static site with ~0 traffic; stop/start is fine |
| AWS CodeDeploy / ECS | Heavy managed services; SSH/SCP is simpler for one instance |
| AWS Route 53 | $0.50/mo hosted zone; GoDaddy A record to Elastic IP is sufficient |
| EC2 arm64 (Graviton) | Requires cross-compilation; x86_64 matches CI runner without extra complexity |
| `cargo audit` in CI | Adds external network dependency; false positives can block PRs |
| Code coverage reporting | No value for learning project at this stage |
| Blue/green deployment | Requires 2x infrastructure; future consideration |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| CI-01 | Phase 6 | Pending |
| CI-02 | Phase 6 | Pending |
| CI-03 | Phase 6 | Pending |
| CI-04 | Phase 6 | Pending |
| CI-05 | Phase 6 | Pending |
| CI-06 | Phase 6 | Pending |
| BRANCH-01 | Phase 7 | Pending |
| BRANCH-02 | Phase 7 | Pending |
| INFRA-01 | Phase 8 | Pending |
| INFRA-02 | Phase 8 | Pending |
| INFRA-03 | Phase 8 | Pending |
| DEPLOY-01 | Phase 9 | Pending |
| DEPLOY-02 | Phase 9 | Pending |
| DEPLOY-03 | Phase 9 | Pending |
| DEPLOY-04 | Phase 9 | Pending |
| DEPLOY-05 | Phase 9 | Pending |
| DNS-01 | Phase 10 | Pending |
| DNS-02 | Phase 10 | Pending |
| DNS-03 | Phase 10 | Pending |
| CD-01 | Phase 11 | Pending |
| CD-02 | Phase 11 | Pending |
| CD-03 | Phase 11 | Pending |
| CD-04 | Phase 11 | Pending |
| CD-05 | Phase 11 | Pending |
| DOCS-01 | Phase 12 | Pending |
| DOCS-02 | Phase 12 | Pending |
| DOCS-03 | Phase 12 | Pending |

**Coverage:**
- v1.1 requirements: 26 total
- Mapped to phases: 26
- Unmapped: 0 ✓

---
*Requirements defined: 2026-03-10*
*Last updated: 2026-03-10 after initial v1.1 definition*
