[![CI](https://github.com/ptdecker/kiss-server/actions/workflows/ci.yml/badge.svg)](https://github.com/ptdecker/kiss-server/actions/workflows/ci.yml)

# kiss-server

A from-scratch HTTP/1.1 static file server written in pure Rust with no external dependencies beyond the `log` crate facade. A client can request any static file by path and receive a correct, RFC-compliant HTTP/1.1 response — without crashing, leaking filesystem paths, or serving the wrong content type.

## Build & Run Locally

```bash
cargo build --release
./target/release/kiss-server --root /path/to/webroot
```

The `--root` flag is required and specifies the directory to serve files from. The `--port` flag is optional (defaults to 6502).

```bash
# Example: serve the current directory on port 8080
./target/release/kiss-server --root . --port 8080
```

## Deployment

kiss-server is live at [http://ptodd.org/](http://ptodd.org/) on an EC2 t3.micro instance (54.83.192.65).

To deploy, promote main to the prod branch:

```bash
git push origin origin/main:prod
```

This triggers the CD pipeline which builds a release binary, deploys it to EC2, verifies the service is running, and creates a GitHub Release. See [docs/ci-cd.md](docs/ci-cd.md) for full pipeline documentation.

## Architecture

kiss-server uses a Handler, Context, and Router abstraction with a fixed thread pool for concurrent connections. Requests are parsed, routed to handlers by URL path, and static files are served with binary-safe reads, MIME detection, and path traversal prevention. The entire server is built on Rust's standard library — no async runtime, no frameworks.

See [docs/design.md](docs/design.md) for the full architecture walkthrough.

## Scripts

Automation scripts for infrastructure provisioning, CI/CD setup, and deployment verification live in the `scripts/` directory.

See [scripts/README.md](scripts/README.md) for details on each script.
