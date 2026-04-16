set dotenv-load

# Install git hooks from scripts/
install-hooks:
    @bash scripts/install-hooks.sh

# Lint the project
lint:
    @cargo fmt
    @cargo clippy

# Build web site
build: lint
    @cargo build

# Run the web site from source (pass args after --: just run -- --config kiss-server.toml)
run *ARGS: build
    @cargo run -- {{ARGS}}

# Test the web site from source
test: build
    @cargo test

# Build the documentation
build-docs: lint
    @cargo doc --lib --document-private-items --no-deps

# View the documentation
docs: build-docs
    @open target/doc/kiss-server/index.html

# Verify DNS records for ptodd.org
verify-dns:
    @./scripts/verify-dns.sh

# Apply branch protection rules to main
branch-protection:
    @./scripts/setup-branch-protection.sh

# Apply branch protection rules to prod
prod-protection:
    @./scripts/setup-prod-protection.sh

# Bump version in Cargo.toml and regenerate Cargo.lock
bump VERSION:
    @bash scripts/bump-version.sh {{VERSION}}

# Tag a release and deploy to production
deploy VERSION:
    #!/usr/bin/env bash
    set -euo pipefail
    bash scripts/pre-deploy-check.sh {{VERSION}}
    TAG="v{{VERSION}}"
    if git tag | grep -qx "$TAG"; then git tag -f "$TAG"; else git tag "$TAG"; fi
    git push origin "$TAG" --force-with-lease=refs/tags/"$TAG" 2>/dev/null || git push origin "$TAG" --force
    git push origin main:prod

# Show status of the last 3 CD pipeline runs
deploy-status:
    @gh run list --workflow=cd.yml --limit=3 \
      --json status,conclusion,createdAt,url \
      --jq '.[] | [(.conclusion // .status), .createdAt, .url] | @tsv'

# View last 100 lines of production logs (requires .env with EC2_HOST set)
logs:
    #!/usr/bin/env bash
    set -euo pipefail
    SSH_ARGS=""
    if [ -n "${EC2_SSH_KEY:-}" ]; then SSH_ARGS="-i ${EC2_SSH_KEY}"; fi
    # Uses sudo because ec2-user may not be in systemd-journal group on Amazon Linux 2023
    ssh $SSH_ARGS "$EC2_HOST" sudo journalctl -u kiss-server -n 100 --no-pager

# Stream production logs live — Ctrl-C to stop (requires .env with EC2_HOST set)
logs-follow:
    #!/usr/bin/env bash
    set -euo pipefail
    SSH_ARGS=""
    if [ -n "${EC2_SSH_KEY:-}" ]; then SSH_ARGS="-i ${EC2_SSH_KEY}"; fi
    # Uses sudo because ec2-user may not be in systemd-journal group on Amazon Linux 2023
    ssh $SSH_ARGS "$EC2_HOST" sudo journalctl -u kiss-server -f
