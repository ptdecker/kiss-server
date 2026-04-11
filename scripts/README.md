# Scripts

Automation scripts for infrastructure provisioning, CI/CD setup, and deployment verification.

## Script Reference

| Script                       | Purpose                                                                                                                                                                                                                               | Run Where         | Prerequisites                                            |
|------------------------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|-------------------|----------------------------------------------------------|
| `ci.sh`                      | Runs local CI checks: `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo build --locked`, `cargo test --locked`                                                                                        | Developer machine | Rust toolchain (version pinned in `rust-toolchain.toml`) |
| `install-kiss-server.sh`     | Idempotent EC2 setup: creates 512MB swap, installs git + gcc + Rust, clones repo, builds release binary, installs to `/usr/local/bin/kiss-server`, creates `kiss-server` system user, writes systemd unit, enables and starts service | EC2 (via SSH)     | Fresh Amazon Linux 2023 instance                         |
| `setup-aws-infra.sh`         | Provisions EC2 t3.micro (Amazon Linux 2023, x86_64), Security Group (ports 22 + 80), and Elastic IP in us-east-1                                                                                                                      | Developer machine | AWS CLI v2 with `kiss` profile configured                |
| `setup-branch-protection.sh` | Creates or updates "Protect main" GitHub Ruleset requiring PR + passing CI status check                                                                                                                                                  | Developer machine | `gh` CLI authenticated                                   |
| `setup-cd-iam.sh`            | Creates IAM user `kiss-cd-cloudfront` with scoped `cloudfront:CreateInvalidation` policy, generates access key, prints `gh secret set` commands                                                                                      | Developer machine | AWS CLI v2 with `kiss` profile, `gh` CLI                |
| `setup-cloudfront.sh`        | Provisions CloudFront distribution with EC2:80 HTTP-only origin, HTTPS termination (ACM cert), HTTP-to-HTTPS redirect, Host header forwarding, TLSv1.2_2021, compression, TTL=0. Polls until Deployed. Writes `docs/aws-resources.md`. | Developer machine | AWS CLI v2 with `kiss` profile configured                |
| `setup-iptables.sh`          | Configures iptables: port 80 to 8080 redirect + INPUT ACCEPT rules for both ports, persists rules via iptables-services                                                                                                               | EC2 (via SSH)     | EC2 instance running                                     |
| `setup-prod-protection.sh`   | Creates or updates "Protect prod" GitHub Ruleset (deletion protection + non-fast-forward rules)                                                                                                                                          | Developer machine | `gh` CLI authenticated                                   |
| `setup-webroot.sh`           | Creates `/var/www/ptodd.org/` directory and writes Hello World `index.html`                                                                                                                                                           | EC2 (via SSH)     | EC2 instance running                                     |
| `verify-dns.sh`              | Smoke tests: CloudFront CNAME resolution, HTTPS content, CloudFront headers, and apex redirect chain for ptodd.org and www.ptodd.org                                                                                                   | Developer machine | `dig` and `curl` available                               |

## DNS Cutover

Step-by-step operator runbook for cutting `www.ptodd.org` and `ptodd.org` over to CloudFront. All DNS changes are made in the GoDaddy control panel UI.

**CloudFront distribution:** `d3ahc2eiiqz0iu.cloudfront.net` (ID: `E2JG60F8N1ZBAK`)

### Pre-Cutover: Lower TTL

1. Log in to GoDaddy DNS management for `ptodd.org`
2. Find the `www` CNAME record (currently pointing to `@`)
3. Edit the record and set TTL to **600 seconds** (or lowest available)
4. Save the change
5. **Wait at least 1 hour** before proceeding — this allows resolvers caching the old (higher) TTL to flush

### Step 1: Change www CNAME to CloudFront

1. In GoDaddy DNS management for `ptodd.org`, find the `www` CNAME record
2. Change the value from `@` to `d3ahc2eiiqz0iu.cloudfront.net`
3. Keep TTL at 600 seconds (or lowest available)
4. Save the change
5. Wait 5-10 minutes for propagation, then verify:

```bash
dig +short www.ptodd.org CNAME
# Expected: d3ahc2eiiqz0iu.cloudfront.net.
```

### Step 2: Enable Apex Domain Forwarding

1. In GoDaddy control panel, go to **Domain** settings (not DNS management)
2. Find **Forwarding** (sometimes under "Forwarding & Parking" or similar)
3. Add forwarding for `ptodd.org`:
   - **Forward to:** `https://www.ptodd.org`
   - **Redirect type:** Permanent (301)
   - **Forward settings:** Forward only (not masking)
4. Save the change
5. GoDaddy will replace the apex A record with their forwarding servers (this may take 5-30 minutes)
6. Verify:

```bash
curl -sI -L --max-time 15 http://ptodd.org/ | head -5
# Expected: HTTP/1.1 301 Moved Permanently (redirect to https://www.ptodd.org/)
```

### Step 3: Run Full Verification

```bash
bash scripts/verify-dns.sh
```

All 6 checks should pass. If any fail, wait 10-15 minutes for DNS propagation and re-run.

### Rollback

If CloudFront is not serving correctly after the CNAME switch:

1. In GoDaddy DNS management, change `www` CNAME value back to `ec2-54-83-192-65.compute-1.amazonaws.com`
2. If apex forwarding was enabled, remove it and re-add the `@` A record pointing to `54.83.192.65`
3. Wait for DNS propagation at the reduced TTL (up to 10 minutes at 600s TTL)
4. EC2 is still live on port 80 — the site returns immediately once DNS propagates

## Verifying CloudFront Cache Invalidation

After the CD pipeline GitHub Secrets are stored (`CF_AWS_ACCESS_KEY_ID`, `CF_AWS_SECRET_ACCESS_KEY`, `CLOUDFRONT_DISTRIBUTION_ID`), verify the invalidation step works end-to-end by triggering a real prod deploy:

```bash
git fetch origin
git checkout prod && git merge main --no-edit
just deploy <VERSION>   # e.g. just deploy 1.2.0
```

Then open the Actions log at `github.com/ptdecker/kiss-server/actions` and look for the **"Invalidate CloudFront cache"** step. A successful run prints:

```
CloudFront invalidation created: I<UUID>
```

If the step fails, check that all three CF_ secrets are present:

```bash
gh secret list --repo ptdecker/kiss-server
```

Re-run `bash scripts/setup-cd-iam.sh E2JG60F8N1ZBAK` if any are missing.

## Usage Notes

- EC2 scripts are executed via SSH:
  `scp script.sh ec2-user@54.83.192.65:/tmp/ && ssh ec2-user@54.83.192.65 'bash /tmp/script.sh'`
- All scripts use `set -euo pipefail` and exit non-zero on failure.
- All scripts are idempotent — safe to re-run if a step was partially completed.
