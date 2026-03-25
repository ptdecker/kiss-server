# CI/CD Pipeline

kiss-server uses GitHub Actions for continuous integration and continuous deployment.

## How CI Works

- **Trigger:** Every push to `main` and every pull request targeting `main` (configured in
  `.github/workflows/ci.yml`)
- **Runner:** `ubuntu-latest`
- **Toolchain:** Rust 1.93.1 pinned via `dtolnay/rust-toolchain@master` (version also in
  `rust-toolchain.toml` for local dev)
- **Cache:** `Swatinem/rust-cache@v2` with `cache-on-failure: true` — caches Cargo registry and
  build artifacts between runs
- **Steps:** The workflow runs `scripts/ci.sh`, which executes in order:
    1. `cargo fmt --check` — formatting
    2. `cargo clippy --locked --all-targets -- -D warnings` — lint (warnings are errors)
    3. `cargo build --locked` — build
    4. `cargo test --locked` — tests
- **Branch protection:** The `ci` job (job name in ci.yml) is a required status check on `main`. PRs
  cannot merge until CI passes.
- **Run locally:** `./scripts/ci.sh` runs the same checks on your machine.

## How to Deploy

- **Trigger:** Push a semver tag (e.g., `v1.2.0`) to the repository — the CD pipeline triggers on
  tags matching `v[0-9]+.[0-9]+.[0-9]+`
- **Promote command:** `just deploy 1.2.0` — tags `v1.2.0`, validates the CHANGELOG entry, and
  pushes the tag to trigger the pipeline. Requires a matching entry in `CHANGELOG.md`.
- **What the CD pipeline does:**
    1. Validates that `CHANGELOG.md` contains an entry for the tag version (fails fast if missing)
    2. Checks out the code and builds a release binary (`cargo build --release --locked`)
    3. SCPs the binary to `/tmp/kiss-server-new` on EC2 (`ec2-user@54.83.192.65`)
    4. Stops the `kiss-server` systemd service
    5. Moves the binary to `/usr/local/bin/kiss-server` (atomic replacement)
    6. Starts the service
    7. Verifies the service is active (`sudo systemctl is-active kiss-server`) — the pipeline fails
       if this check fails
    8. Creates a GitHub Release tagged `vX.Y.Z (short-sha)` with the compiled binary attached
- **Verify after deploy:**
    - Check the GitHub Actions CD run for green status
    - `curl -s -o /dev/null -w '%{http_code}' http://ptodd.org/` should return `200`
    - SSH and check: `ssh ec2-user@54.83.192.65 "sudo systemctl is-active kiss-server"` should print
      `active`

## Checking EC2 Health

SSH target: `ec2-user@54.83.192.65`

```bash
# Service status (detailed)
ssh ec2-user@54.83.192.65 "sudo systemctl status kiss-server"

# Quick active check (exits 0 if running)
ssh ec2-user@54.83.192.65 "sudo systemctl is-active kiss-server"

# Recent logs (last 50 lines)
ssh ec2-user@54.83.192.65 "sudo journalctl -u kiss-server -n 50"

# Restart service
ssh ec2-user@54.83.192.65 "sudo systemctl restart kiss-server"
```

The service runs as the `kiss-server` system user. Binary location: `/usr/local/bin/kiss-server`.
Webroot: `/var/www/ptodd.org/`. Port 8080 (iptables redirects port 80 to 8080).

## Setup from Scratch

High-level steps to recreate the full infrastructure and pipeline from zero. See
[scripts/README.md](../scripts/README.md) for details on each script.

1. **AWS infrastructure:** Run `scripts/setup-aws-infra.sh` — provisions EC2 t3.micro, Security
   Group, and Elastic IP. Requires AWS CLI v2 with the `kiss` profile configured for account
   859953692821 in us-east-1.

2. **EC2 service setup:** SSH to the instance and run three scripts in order:
    - `scripts/install-kiss-server.sh` — installs Rust, builds kiss-server, creates systemd unit
    - `scripts/setup-webroot.sh` — creates `/var/www/ptodd.org/` with Hello World index.html
    - `scripts/setup-iptables.sh` — configures port 80 to 8080 redirect

3. **DNS:** Configure GoDaddy DNS: A record for `@` pointing to the Elastic IP, CNAME for `www`
   pointing to `@`. Verify with `scripts/verify-dns.sh`.

4. **GitHub secrets:** Add two repository secrets:
    - `EC2_SSH_KEY` — private key content for SSH access to EC2
    - `EC2_KNOWN_HOSTS` — EC2 host fingerprint (use `ssh-keyscan` output, not runtime TOFU)

5. **Branch protection:** Run `scripts/setup-branch-protection.sh` (protects `main`) and
   `scripts/setup-prod-protection.sh` (protects `prod`). Both require `gh` CLI authenticated.

6. **Create prod branch:** `git push origin origin/main:prod` — this also triggers the first CD
   deployment.

## Just Recipes

Convenience recipes for pipeline and infrastructure operations:

- `just deploy <VERSION>` — tag the current commit as `vVERSION` and push to prod, triggering the CD pipeline (e.g., `just deploy 1.2.0`)
- `just verify-dns` — run DNS smoke tests for ptodd.org and www.ptodd.org
- `just branch-protection` — apply or update main branch protection rules via GitHub API
- `just prod-protection` — apply or update prod branch protection rules via GitHub API
