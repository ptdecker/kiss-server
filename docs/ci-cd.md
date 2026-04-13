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
    9. Creates a CloudFront cache invalidation for `/*` (fire-and-forget — clears CDN cache)
- **Verify after deploy:**
    - Check the GitHub Actions CD run for green status
    - `curl -s -o /dev/null -w '%{http_code}' https://www.ptodd.org/` should return `200`
    - SSH and check: `ssh ec2-user@54.83.192.65 "sudo systemctl is-active kiss-server"` should print
      `active`
    - Check CD run log for CloudFront invalidation ID (confirms cache was cleared)

## CloudFront Cache Invalidation

Every successful deploy automatically invalidates the CloudFront cache so visitors see the latest
content without waiting for TTL expiry.

### What Happens

After the CD pipeline deploys the binary and verifies the service is active (step 7), an additional
step runs:

```bash
aws cloudfront create-invalidation \
  --distribution-id "$CLOUDFRONT_DISTRIBUTION_ID" \
  --paths "/*"
```

This creates a wildcard invalidation (`/*`) that clears all cached objects at every CloudFront edge
location. The invalidation runs fire-and-forget — the pipeline does not wait for it to complete
(invalidations typically finish within 60 seconds).

### IAM Least-Privilege Setup

The invalidation step uses a dedicated IAM user (`kiss-cd-cloudfront`) with a policy scoped to a
single action on a single resource:

- **Action:** `cloudfront:CreateInvalidation`
- **Resource:** `arn:aws:cloudfront::859953692821:distribution/E2JG60F8N1ZBAK`

The credentials are stored as GitHub Secrets:
- `CF_AWS_ACCESS_KEY_ID` — access key for kiss-cd-cloudfront
- `CF_AWS_SECRET_ACCESS_KEY` — secret key for kiss-cd-cloudfront
- `CLOUDFRONT_DISTRIBUTION_ID` — distribution ID (`E2JG60F8N1ZBAK`)

These secrets are scoped to the invalidation step only (step-level `env` block in cd.yml), not
exposed to the SSH deploy steps.

### Verifying Invalidation

After a deploy, check the GitHub Actions CD run log for the invalidation step output. It prints the
invalidation ID. To check status:

```bash
aws cloudfront get-invalidation \
  --distribution-id E2JG60F8N1ZBAK \
  --id <INVALIDATION_ID> \
  --profile kiss
```

Status progresses from `InProgress` to `Completed`. You can also verify by checking that
`https://www.ptodd.org/` serves the updated content after deploy.

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

3. **ACM certificate:** Request a public cert in us-east-1 for `ptodd.org` and `www.ptodd.org`
   via AWS Certificate Manager. Validate via DNS (add the CNAME records GoDaddy side).

4. **CloudFront distribution:** Create a distribution with the EC2 Elastic IP as origin (HTTP
   port 80). Attach the ACM cert. Set `www.ptodd.org` as the alternate domain name. Enable
   `Redirect HTTP to HTTPS` viewer policy. Run `scripts/setup-security-group.sh` to restrict
   EC2 port 80 to the CloudFront managed prefix list.

5. **DNS cutover:** In GoDaddy, add a CNAME for `www` pointing to the CloudFront domain
   (e.g., `d3ahc2eiiqz0iu.cloudfront.net`). Use a forwarding rule for the apex (`ptodd.org`)
   to redirect to `https://www.ptodd.org/`. Verify with `scripts/verify-dns.sh`.

6. **GitHub secrets:** Add five repository secrets:
    - `EC2_SSH_KEY` — private key content for SSH access to EC2
    - `EC2_KNOWN_HOSTS` — EC2 host fingerprint (use `ssh-keyscan` output, not runtime TOFU)
    - `CLOUDFRONT_DISTRIBUTION_ID` — the distribution ID (e.g., `E2JG60F8N1ZBAK`)
    - `CF_AWS_ACCESS_KEY_ID` — IAM access key for the least-privilege CloudFront invalidation role
    - `CF_AWS_SECRET_ACCESS_KEY` — corresponding IAM secret key

7. **Branch protection:** Run `scripts/setup-branch-protection.sh` (protects `main`) and
   `scripts/setup-prod-protection.sh` (protects `prod`). Both require `gh` CLI authenticated.

8. **Create prod branch:** `git push origin origin/main:prod` — this also triggers the first CD
   deployment.

## Just Recipes

Convenience recipes for pipeline and infrastructure operations:

- `just deploy <VERSION>` — tag the current commit as `vVERSION` and push to prod, triggering the CD pipeline (e.g., `just deploy 1.2.0`)
- `just verify-dns` — run DNS smoke tests for ptodd.org and www.ptodd.org
- `just branch-protection` — apply or update main branch protection rules via GitHub API
- `just prod-protection` — apply or update prod branch protection rules via GitHub API
