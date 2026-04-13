# Lint the project
lint:
    @cargo fmt
    @cargo clippy

# Build web site
build: lint
    @cargo build

# Run the web site from source
run: build
    @cargo run

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
