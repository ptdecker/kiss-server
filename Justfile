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
    @open target/doc/ptodd/index.html
