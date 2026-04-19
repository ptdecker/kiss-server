[![CI](https://github.com/ptdecker/kiss-server/actions/workflows/ci.yml/badge.svg)](https://github.com/ptdecker/kiss-server/actions/workflows/ci.yml)

# kiss-server

A from-scratch HTTP/1.1 static file server written in pure Rust with no external dependencies beyond
the `log` crate facade. A client can request any static file by path and receive a correct,
RFC-compliant HTTP/1.1 response — without crashing, leaking filesystem paths, or serving the wrong
content type.

## Build & Run Locally

```bash
cargo build --release
./target/release/kiss-server --root /path/to/webroot
```

The `--root` flag is required and specifies the directory to serve files from. The `--port` flag is
optional (defaults to 6502).

```bash
# Example: serve the current directory on port 8080
./target/release/kiss-server --root . --port 8080
```

## Development

[Just](https://github.com/casey/just) recipes for common tasks:

```bash
just lint          # Format and lint (cargo fmt + cargo clippy)
just build         # Lint then build
just run           # Lint, build, then run
just test          # Lint, build, then test
just build-docs    # Generate rustdoc
just docs          # Generate and open rustdoc in browser
```

### Auth middleware and local testing

In production, kiss-server runs behind CloudFront + Lambda@Edge. Lambda@Edge validates JWTs and
injects an `X-Authenticated-User` header before the request reaches the origin. The server trusts
this header because only CloudFront can reach the EC2 instance (security group restricts port 80 to
CloudFront IP ranges).

In development there is no Lambda@Edge, so every request gets a 401 unless you work around the
middleware. Two options:

**Option 1 — Disable auth entirely (simplest)**

```bash
KISS_SKIP_AUTH=1 just run
```

The server logs a visible warning at startup: `KISS_SKIP_AUTH set — auth middleware disabled (dev
mode only)`. Never set this in the production systemd unit.

**Option 2 — Pass the header manually (curl / scripts)**

```bash
curl -H "X-Authenticated-User: dev" http://localhost:6502/
```

Useful when you want to test auth-on behaviour (e.g. verifying a 401 is returned without the
header) alongside normal requests.

### Testing plugins locally

Plugins are only activated when using `--config`. To test the URL shortener locally, add a
`[server]` section to `kiss-server.toml` pointing at a static file directory, then run with
`--config`:

```toml
# kiss-server.toml (local testing only — do not commit default_root)
[server]
default_root = "docs"

[[plugin]]
name = "url-shortener"
```

```bash
KISS_SKIP_AUTH=1 ./target/debug/kiss-server --config kiss-server.toml --port 8080
```

The three hardcoded seed codes are available immediately after startup:

| URL | Redirects to |
|-----|-------------|
| `http://localhost:8080/s/gh` | https://github.com/ptdecker |
| `http://localhost:8080/s/rs` | https://www.rust-lang.org |
| `http://localhost:8080/s/hn` | https://news.ycombinator.com |

Plugin state is in-memory and resets on every restart.

## Deployment

kiss-server is live at [https://www.ptodd.org/](https://www.ptodd.org/) on an EC2 t3.micro behind
CloudFront (ACM TLS, cache invalidation on deployment).

To deploy, update CHANGELOG.md with the release notes, then run:

```bash
just deploy 1.2.0  # tags v1.2.0 and pushes to prod
```

This triggers the CD pipeline which builds a release binary, deploys it to EC2, verifies the service
is running, and creates a GitHub Release. See [docs/ci-cd.md](docs/ci-cd.md) for full pipeline
documentation.

## Architecture

The kiss-server uses a Handler, Context, and Router abstraction with a fixed thread pool for
concurrent connections. Requests are parsed, routed to handlers by URL path, and static files are
served with binary-safe reads, MIME detection, and path traversal prevention. The entire server is
built on Rust's standard library — no async runtime, no frameworks.

See [docs/design.md](docs/design.md) for the full architecture walkthrough.

## Scripts

Automation scripts for infrastructure provisioning, CI/CD setup, and deployment verification live in
the `scripts/` directory.

See [scripts/README.md](scripts/README.md) for details on each script.
