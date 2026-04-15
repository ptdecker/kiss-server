# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.4.0] - 2026-04-14

### Added

- Per-request access logging: one structured `info!(target: "access", ...)` line per response with
  peer IP, HTTP version, method, path, Host header, status code, response bytes, and duration_ms
- `just logs` — SSHes to EC2 and shows last 100 lines of the kiss-server journal
- `just logs-follow` — SSHes to EC2 and streams live journal output
- `.env.example` template documenting `EC2_HOST` and `EC2_SSH_KEY` variables

### Changed

- systemd unit sets `Environment=RUST_LOG=info` for structured production log level
- `scripts/install-kiss-server.sh` updated so log level survives every redeployment

## [1.3.0] - 2026-04-13

### Added

- Multi-domain virtual hosting via `Host` header dispatch — the server now routes requests to
  per-domain `StaticFileHandler` instances based on the normalized `Host` header value
- `--config <path>` CLI argument: loads a TOML config file with `[[vhost]]` entries (each
  specifying `domain` and `root`) and an optional `[server]` section with `default_root`
- Hand-rolled TOML config parser (`src/config/`) with no new crate dependencies
- `VhostDispatcher` handler (`src/handlers/vhost.rs`) that routes known domains, serves a
  parked-domain HTML page for unknown hosts, and falls back to `default_root` when configured
- `--config` and `--root` are mutually exclusive; passing both is a startup error

### Changed

- `--root <path>` backward compatibility preserved: synthesizes a `VhostDispatcher` with a single
  default handler, so existing single-root deployments require no changes
- Server startup now dispatches all requests through `VhostDispatcher` regardless of mode

## [1.2.4] - 2026-04-12

### Documentation

- `README.md`: deployment URL updated to `https://www.ptodd.org/` (was HTTP); EC2 IP removed (no
  longer the public entry point behind CloudFront)
- `docs/ci-cd.md`: "Setup from Scratch" section updated for post-CloudFront reality — ACM cert and
  CloudFront distribution steps added, DNS instructions updated from pre-cutover A-record setup to
  post-cutover CNAME-to-CloudFront, GitHub secrets expanded from 2 to 5 (adds
  `CLOUDFRONT_DISTRIBUTION_ID`, `CF_AWS_ACCESS_KEY_ID`, `CF_AWS_SECRET_ACCESS_KEY`)
- `CLAUDE.md`: test count corrected (86 → 88)
- `src/main.rs`: crate-level docstring updated to reflect general-purpose HTTP server with
  CloudFront TLS termination

## [1.2.3] - 2026-04-12

### Changed

- CD pipeline now triggers on semver tag push (`v*.*.*`) instead of prod branch push, eliminating
  the race condition where a branch sync could fire the deployment before a tag existed
- `pre-deploy-check.sh`: added check that local `main` matches `origin/main` before tagging,
  preventing deployment of commits that haven't passed CI
- `just deploy-status` recipe: shows last 3 CD pipeline run outcomes from the CLI
- CD pipeline: added `workflow_dispatch` trigger with tag input for manual re-runs without requiring
  a prod branch push

## [1.2.2] - 2026-04-12

### Added

- `scripts/bump-version.sh`: automates `Cargo.toml` and `Cargo.lock` version bump with a release
  checklist
- `scripts/pre-deploy-check.sh`: validates clean tree, main branch, version consistency, and
  CHANGELOG entry before deploy
- `just bump VERSION` recipe for one-command version bumping
- `just deploy VERSION` now runs pre-deploy checks before tagging

## [1.2.1] - 2026-04-12

### Fixed

- Branch protection script: require PR with one approval and squash-only merges; fix pull_request
  rule schema for GitHub Rulesets API

## [1.2.0] - 2026-04-12

### Infrastructure

- CloudFront distribution for HTTPS termination at edge via ACM certificate (ptodd.org,
  www.ptodd.org)
- ACM certificate in us-east-1: auto-renewing DNS-validated certificate for ptodd.org and
  www.ptodd.org
- CD pipeline: CloudFront cache invalidation (`/*`) after each successful deployment with
  least-privilege IAM credentials
- DNS cutover: www.ptodd.org CNAME to CloudFront; ptodd.org apex 301-forwarded to
  https://www.ptodd.org
- EC2 security group: port 80 restricted to CloudFront managed prefix list (direct public access
  removed)
- Server: `Connection: close` header on all responses and 30-second read timeout for connection
  stability

## [1.1.0] - 2026-03-25

### Added

- Continuous deployment pipeline: push to `prod` branch triggers automated build, deploy to EC2,
  health check, and GitHub Release
- Branch protection: main requires PR with passing CI; prod has deletion and non-fast-forward
  protection
- AWS infrastructure: EC2 t3.micro with Elastic IP (54.83.192.65) and Security Group
- EC2 service setup: systemd service, iptables port 80 to 8080 redirect, non-root execution
- DNS configuration: ptodd.org and www.ptodd.org routed to EC2 via GoDaddy
- CI pipeline: GitHub Actions workflow running fmt, clippy, build, and test on every push and PR
- Architecture documentation in docs/design.md
- CI and CD pipeline documentation in docs/ci-cd.md
- Scripts reference documentation in scripts/README.md
- CI build status badge in README.md

### Changed

- Cargo package renamed from `ptodd` to `kiss-server` (binary name matches deployed service)
- README.md rewritten with build instructions, deployment guide, and architecture summary

## [1.0.0] - 2026-03-10

### Added

- HTTP/1.1 static file server built from scratch in pure Rust (stdlib + `log` crate only)
- Handler, Context, and Router abstraction for a request pipeline
- Fixed-size thread pool for concurrent connection handling
- Static file serving with binary-safe reads and MIME detection (10 types plus octet-stream
  fallback)
- Path traversal prevention via canonicalizing and starts_with check
- Percent-encoding support for URL paths (RFC 3986 Section 2.1)
- Directory requests serve index.html automatically
- Custom error types with Result propagation (no unwrap on unhappy paths)
- RFC 9110 compliant Date header on every response
- Request header limit (100 lines, 431 on exceed)
- CLI arguments: `--root` (required) and `--port` (optional, default 6502)
