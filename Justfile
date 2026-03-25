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
    git tag v{{VERSION}}
    git push origin v{{VERSION}} origin/main:prod
