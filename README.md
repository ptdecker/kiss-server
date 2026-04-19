[![CI](https://github.com/ptdecker/kiss-server/actions/workflows/ci.yml/badge.svg)](https://github.com/ptdecker/kiss-server/actions/workflows/ci.yml)

# kiss-server

A from-scratch HTTP/1.1 static file server written in pure Rust with no external dependencies beyond
the `log` crate facade. A client can request any static file by path and receive a correct,
RFC-compliant HTTP/1.1 response — without crashing, leaking filesystem paths, or serving the wrong
content type. The kiss-server also supports a pre-dispatch middleware chain for cross-cutting
concerns (authentication, rate limiting) and a plugin system for extending the server with
prefix-routed request handlers.

## Build & Run Locally

```bash
cargo build --release
./target/release/kiss-server --root /path/to/webroot
```

The `--root` flag is required and specifies the directory to serve files from. The `--port` flag is
optional (defaults to 6502 as a tribute to the
[MOS 6502](https://en.wikipedia.org/wiki/MOS_Technology_6502)).

```bash
# Example: serve the current directory on the more traditional port 8080
./target/release/kiss-server --root . --port 8080
```

## Development

The [Just](https://github.com/casey/just) command runner has recipes for common tasks:

```bash
just --list        # List all available recipes

just lint          # Format and lint (cargo fmt + cargo clippy)
just build         # Lint then build
just run           # Lint, build, then run
just test          # Lint, build, then test
just build-docs    # Generate rustdoc
just docs          # Generate and open rustdoc in browser
```

### Auth middleware and local testing

In production, the kiss-server is designed to run behind CloudFront plus Lambda@Edge. Lambda@Edge
validates JWTs and injects an `X-Authenticated-User` header before the request reaches the origin.
The server trusts this header because only CloudFront can reach port 80 on the EC2 instance (the
security group restricts port 80 to the CloudFront managed prefix list; port 22 remains open for
SSH administration).

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

Useful when you want to test auth-on behavior (e.g., verifying a 401 is returned without the
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

| URL                          | Redirects to                 |
|------------------------------|------------------------------|
| `http://localhost:8080/s/gh` | https://github.com/ptdecker  |
| `http://localhost:8080/s/rs` | https://www.rust-lang.org    |
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
concurrent connections. The request lifecycle is:

1. **Middleware chain** — runs before dispatch; each middleware may inspect/mutate the request
   context or short-circuit with a response (e.g., 401 Unauthorized). Named routes can be exempted
   from the chain (public routes). The built-in auth middleware validates the `X-Authenticated-User`
   header injected by Lambda@Edge in production.
2. **Router dispatch** — matches the method and path to the first registered handler. Exact routes
   take priority over prefix routes; path traversal and malformed percent-sequences are rejected
   with 404.
3. **Handlers** — produce the response. The built-in `StaticFileHandler` serves files with
   binary-safe reads, MIME detection, and path traversal prevention.

**Plugin system** — optional prefix-routed handlers loaded at startup via `--config`. Plugins are
built against the `kiss-plugin-sdk` crate (shared types: `Handler`, `KissPlugin`, `Context`,
`Request`, `Response`) and wired into the router under their declared path prefix. The bundled
`kiss-url-shortener` plugin (`/s/<code>`) is the reference implementation.

The entire server is built on Rust's standard library — no async runtime, no frameworks.

See [docs/design.md](docs/design.md) for the full architecture walkthrough.

## Scripts

Automation scripts for infrastructure provisioning, CI/CD setup, and deployment verification live in
the `scripts/` directory.

See [scripts/README.md](scripts/README.md) for details on each script.
