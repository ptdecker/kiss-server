# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Infrastructure

- CloudFront distribution for HTTPS termination at edge via ACM certificate (ptodd.org, www.ptodd.org)
- ACM certificate in us-east-1: auto-renewing DNS-validated certificate for ptodd.org and www.ptodd.org
- CD pipeline: CloudFront cache invalidation (`/*`) after each successful deploy with least-privilege IAM credentials
- DNS cutover: www.ptodd.org CNAME to CloudFront; ptodd.org apex 301-forwarded to https://www.ptodd.org
- EC2 security group: port 80 restricted to CloudFront managed prefix list (direct public access removed)
- Server: `Connection: close` header on all responses and 30-second read timeout for connection stability

## [1.1.0] - 2026-03-25

### Added

- Continuous deployment pipeline: push to `prod` branch triggers automated build, deploy to EC2, health check, and GitHub Release
- Branch protection: main requires PR with passing CI; prod has deletion and non-fast-forward protection
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
- Handler, Context, and Router abstraction for request pipeline
- Fixed-size thread pool for concurrent connection handling
- Static file serving with binary-safe reads and MIME detection (10 types plus octet-stream fallback)
- Path traversal prevention via canonicalize and starts_with check
- Percent-encoding support for URL paths (RFC 3986 Section 2.1)
- Directory requests serve index.html automatically
- Custom error types with Result propagation (no unwrap on unhappy paths)
- RFC 9110 compliant Date header on every response
- Request header limit (100 lines, 431 on exceed)
- CLI arguments: `--root` (required) and `--port` (optional, default 6502)
