# Scripts

Automation scripts for infrastructure provisioning, CI/CD setup, and deployment verification.

## Script Reference

| Script                       | Purpose                                                                                                                                                                                                                               | Run Where         | Prerequisites                                            |
|------------------------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|-------------------|----------------------------------------------------------|
| `bump-version.sh`            | Updates `Cargo.toml` to the given version and regenerates `Cargo.lock`. Run on a feature branch before updating `CHANGELOG.md` and opening a PR. Prints a checklist of remaining release steps.                                      | Developer machine | Rust toolchain                                           |
| `ci.sh`                      | Runs local CI checks: `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo build --locked`, `cargo test --locked`                                                                                        | Developer machine | Rust toolchain (version pinned in `rust-toolchain.toml`) |
| `install-kiss-server.sh`     | Idempotent EC2 setup: creates 512MB swap, installs git + gcc + Rust, clones repo, builds release binary, installs to `/usr/local/bin/kiss-server`, creates `kiss-server` system user, writes systemd unit, enables and starts service | EC2 (via SSH)     | Fresh Amazon Linux 2023 instance                         |
| `setup-aws-infra.sh`         | Provisions EC2 t3.micro (Amazon Linux 2023, x86_64), Security Group (ports 22 + 80), and Elastic IP in us-east-1                                                                                                                      | Developer machine | AWS CLI v2 with `kiss` profile configured                |
| `pre-deploy-check.sh`        | Validates pre-deploy conditions: clean working tree, on `main`, `Cargo.toml` version matches the deploy version, and `CHANGELOG.md` has a matching entry. Called automatically by `just deploy`.                                      | Developer machine | Rust toolchain, git                                      |
| `setup-branch-protection.sh` | Creates or updates "Protect main" GitHub Ruleset requiring PR + passing CI status check                                                                                                                                                  | Developer machine | `gh` CLI authenticated                                   |
| `setup-iptables.sh`          | Configures iptables: port 80 to 8080 redirect + INPUT ACCEPT rules for both ports, persists rules via iptables-services                                                                                                               | EC2 (via SSH)     | EC2 instance running                                     |
| `setup-prod-protection.sh`   | Creates or updates "Protect prod" GitHub Ruleset (deletion protection + non-fast-forward rules)                                                                                                                                          | Developer machine | `gh` CLI authenticated                                   |
| `setup-security-group.sh`    | Restricts EC2 port 80 inbound to CloudFront managed prefix list (`com.amazonaws.global.cloudfront.origin-facing`), removes `0.0.0.0/0` open access. Idempotent with built-in HTTPS verification.                                     | Developer machine | AWS CLI v2 with `kiss` profile configured                |
| `setup-webroot.sh`           | Creates `/var/www/ptodd.org/` directory and writes Hello World `index.html`                                                                                                                                                           | EC2 (via SSH)     | EC2 instance running                                     |
| `verify-dns.sh`              | Smoke tests: A record resolution, CNAME resolution, HTTP content check for ptodd.org and www.ptodd.org                                                                                                                                | Developer machine | `dig` and `curl` available                               |

## Usage Notes

- EC2 scripts are executed via SSH:
  `scp script.sh ec2-user@54.83.192.65:/tmp/ && ssh ec2-user@54.83.192.65 'bash /tmp/script.sh'`
- All scripts use `set -euo pipefail` and exit non-zero on failure.
- All scripts are idempotent — safe to re-run if a step was partially completed.
