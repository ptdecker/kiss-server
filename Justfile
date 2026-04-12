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

# Tag a release and deploy to production
deploy VERSION:
    #!/usr/bin/env bash
    set -euo pipefail
    TAG="v{{VERSION}}"
    # If tag already exists locally, move it to HEAD
    if git tag | grep -qx "$TAG"; then
        git tag -f "$TAG"
    else
        git tag "$TAG"
    fi
    # Push tag (force in case it already exists on remote) and update prod
    git push origin "$TAG" --force-with-lease=refs/tags/"$TAG" 2>/dev/null || git push origin "$TAG" --force
    git push origin main:prod
