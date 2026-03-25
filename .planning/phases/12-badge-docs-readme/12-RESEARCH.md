# Phase 12: Badge, Docs, README - Research

**Researched:** 2026-03-24
**Domain:** Markdown documentation, GitHub Actions badge syntax, project documentation structure
**Confidence:** HIGH

## Summary

Phase 12 is a documentation-only phase. No code changes. No external dependencies. No new tools are required. All raw material already exists in the repository — the CI and CD workflow files, all eight scripts, PROJECT.md architecture decisions, and the live infrastructure details captured in prior phase contexts.

The primary technical question for this phase is the exact GitHub Actions badge syntax. This is a well-established pattern with no ambiguity: the badge SVG URL is derived directly from the workflow filename, and the CONTEXT.md has already confirmed both the URL and link target. Everything else is prose authoring: reading the existing sources, synthesizing them into four new files, and writing clearly for a dual audience (future-self + external contributor).

The planner's job is to sequence four file writes (README.md replacement, docs/ci-cd.md, docs/design.md, scripts/README.md) and ensure each draws from the correct canonical source.

**Primary recommendation:** Write the four files in dependency order — scripts/README.md first (no dependencies), then docs/design.md (draws from PROJECT.md), then docs/ci-cd.md (draws from ci.yml, cd.yml, scripts/README.md), then README.md last (links to all three prior files).

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**CI Badge (DOCS-01)**
- D-01: Badge placed at the very top of README.md, before the `# title` heading — standard OSS convention, most visible placement
- D-02: Badge is a clickable link pointing to the GitHub Actions CI runs page (not a static image)

**docs/ci-cd.md (DOCS-02)**
- D-03: Audience: dual-purpose — future-me quick reference AND external contributor onboarding (both equally)
- D-04: Four required sections:
  1. How CI works + triggers (what runs on push/PR, how to see results)
  2. How to deploy via prod branch (promote main → prod, what CD does, how to verify)
  3. How to check EC2 health (SSH commands, systemctl status)
  4. Setup from scratch (GitHub secrets, branch protection, AWS prerequisites)
- D-05: "Setup from scratch" section uses high-level steps + references to scripts — no copy-pasting script contents inline; the scripts directory README is the authoritative script reference

**README.md (DOCS-03)**
- D-06: Full replacement — current content is too outdated (wrong title, old setup instructions, no CI/CD mention)
- D-07: Top-level README sections:
  1. CI badge (top, before title per D-01)
  2. What it is + core value (project description, no-external-deps philosophy)
  3. Build & run locally (`cargo build --release`, `--root` flag, example)
  4. Deployment overview (live at ptodd.org on EC2, how to deploy via prod, link to docs/ci-cd.md)
  5. Architecture (brief summary + link to docs/design.md)
  6. Scripts (brief mention + link to scripts/README.md)
- D-08: New `scripts/README.md` — describes each of the 8 scripts in scripts/, what each does and when to use it; top-level README references this file

**docs/design.md (new file)**
- D-09: Detailed architecture walkthrough lives in `docs/design.md` (separate from README); covers Handler/Context/Router pattern, thread pool model, error handling approach, key design decisions
- D-10: README.md has a brief architecture summary (2-4 sentences) with a link to `docs/design.md` for deeper reading

### Claude's Discretion
- Exact badge markdown syntax (standard GitHub Actions badge format from ci.yml workflow name)
- Exact heading structure and prose within each doc
- Whether docs/design.md includes the key decisions table from PROJECT.md or a curated subset
- Tone and length of each section within the constraints above

### Deferred Ideas (OUT OF SCOPE)
- None — discussion stayed within phase scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| DOCS-01 | README.md displays a GitHub Actions build status badge showing CI pass/fail | Badge URL and link target confirmed from ci.yml workflow name. Exact markdown syntax documented in Standard Stack section. |
| DOCS-02 | `docs/ci-cd.md` documents how the CI and CD pipelines work and how to use them | ci.yml and cd.yml fully read and summarized. All four required sections have source material identified. |
| DOCS-03 | README.md is updated to reflect the project's current state, how to build/run, and how deployment works | PROJECT.md, ci.yml, cd.yml, all scripts read. Current README.md deficiencies identified. All replacement content material is available. |
</phase_requirements>

## Standard Stack

### Core

| Item | Version/Format | Purpose | Why Standard |
|------|----------------|---------|--------------|
| GitHub Actions badge | SVG via `badge.svg` URL | Visual CI status indicator | GitHub-native, no third-party service required |
| Markdown | CommonMark | Documentation format | GitHub renders it natively; all existing docs use it |

### Badge Syntax (locked — D-01, D-02)

The badge is a clickable image link. The workflow name in ci.yml is `CI` and the file is `ci.yml`.

```markdown
[![CI](https://github.com/ptdecker/kiss-server/actions/workflows/ci.yml/badge.svg)](https://github.com/ptdecker/kiss-server/actions/workflows/ci.yml)
```

Placement: This line goes at the very top of README.md, **before** the `# title` heading.

**Badge URL format:** `https://github.com/{owner}/{repo}/actions/workflows/{workflow-file}/badge.svg`
**Badge link target:** `https://github.com/{owner}/{repo}/actions/workflows/{workflow-file}`

Confidence: HIGH — these URLs are derived directly from the workflow filename and repo path. The workflow file is `.github/workflows/ci.yml` and repo is `ptdecker/kiss-server`. No external service required; GitHub serves this SVG natively.

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| GitHub-native badge SVG | shields.io badge | shields.io adds third-party dependency and can have latency; GitHub-native is authoritative and always current |
| Badge before title | Badge after title | Before-title is the dominant OSS convention; maximally visible without scrolling |

## Architecture Patterns

### File Output Map

```
kiss-server/
├── README.md                 # Replace entirely (D-06)
├── docs/
│   ├── ci-cd.md              # New file (DOCS-02, D-03 through D-05)
│   └── design.md             # New file (D-09, D-10)
└── scripts/
    └── README.md             # New file (D-08)
```

Note: `docs/` directory does not currently exist. It must be created implicitly when docs/ci-cd.md and docs/design.md are written.

### Dependency Order for Writing

Write in this order to avoid forward-reference problems when reviewing:

1. `scripts/README.md` — no dependencies on other new files
2. `docs/design.md` — draws from PROJECT.md Key Decisions table; no dependency on other new docs
3. `docs/ci-cd.md` — references scripts/ README for "setup from scratch" (D-05); depends on step 1
4. `README.md` — links to all three above; depends on steps 1-3

### Pattern: README.md Structure (locked — D-07)

```markdown
[![CI](badge-url)](link-url)

# kiss-server

{what it is + core value — 2-3 sentences}

## Build & Run Locally

{cargo build --release, --root flag, example invocation}

## Deployment

{live at ptodd.org on EC2, deploy command, link to docs/ci-cd.md}

## Architecture

{2-4 sentences: Handler/Context/Router, thread pool, no-external-deps}
See [docs/design.md](docs/design.md) for the full architecture walkthrough.

## Scripts

{1-2 sentences}
See [scripts/README.md](scripts/README.md) for details on each script.
```

### Pattern: docs/ci-cd.md Structure (locked — D-04)

Four required sections in this order (from D-04):

```markdown
# CI/CD Pipeline

## How CI Works

## How to Deploy

## Checking EC2 Health

## Setup from Scratch
```

### Pattern: docs/design.md Structure

Draws from PROJECT.md. Should cover:
- Handler/Context/Router pattern (what each abstraction does)
- Thread pool model (fixed size, synchronous blocking I/O)
- Error handling approach (Result propagation, no unwrap on unhappy paths)
- Key design decisions (from the Key Decisions table in PROJECT.md — curated subset or full table is Claude's discretion per CONTEXT.md)

### Pattern: scripts/README.md Structure

A reference table or list with:
- Script name
- What it does (one sentence)
- When to run it (trigger/context)
- Prerequisites (where non-obvious)

### Anti-Patterns to Avoid

- **Copy-pasting script contents inline into docs/ci-cd.md:** D-05 explicitly prohibits this. Reference the scripts directory README instead.
- **Repeating architecture details in README.md that belong in docs/design.md:** README.md gets 2-4 sentences max (D-10); docs/design.md gets the full treatment.
- **Writing badge as a plain image without a link:** D-02 requires the badge to be a clickable link to the CI runs page.
- **Placing badge below the title:** D-01 requires it before the `# title` heading.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| CI status badge | Custom status endpoint or webhook | GitHub-native badge SVG | Already provided by GitHub; zero maintenance |
| Script documentation table | Long-form prose for each script | Structured table in scripts/README.md | Faster to scan; easier to maintain |

## Source Material Inventory

All raw material for this phase is already available in the repository. Nothing needs to be discovered at runtime.

### For DOCS-01 (CI Badge)

| Fact | Value | Source |
|------|-------|--------|
| Workflow file | `.github/workflows/ci.yml` | Read directly |
| Workflow name | `CI` | `name: CI` in ci.yml line 1 |
| Repo | `ptdecker/kiss-server` | Confirmed in CONTEXT.md |
| Badge SVG URL | `https://github.com/ptdecker/kiss-server/actions/workflows/ci.yml/badge.svg` | CONTEXT.md §Integration Points |
| Badge link target | `https://github.com/ptdecker/kiss-server/actions/workflows/ci.yml` | CONTEXT.md §Integration Points |

### For DOCS-02 (docs/ci-cd.md)

**Section 1 — How CI works + triggers:**
- Trigger: push to `main`, PR targeting `main` (ci.yml lines 3-6)
- Steps: `cargo fmt --check`, `cargo clippy --locked --all-targets -- -D warnings`, `cargo build --locked`, `cargo test --locked` (scripts/ci.sh)
- Toolchain: Rust 1.93.1 pinned via `dtolnay/rust-toolchain@master` (ci.yml lines 17-20)
- Cache: `Swatinem/rust-cache@v2` with `cache-on-failure: true` (ci.yml lines 21-23)
- Job name: `ci` (ci.yml line 13) — this is the required status check name in branch protection

**Section 2 — How to deploy via prod branch:**
- Deploy trigger: push to `prod` branch (cd.yml line 5)
- Promote command: `git push origin origin/main:prod` (confirmed in STATE.md Phase 11 note)
- CD steps: build release binary → SCP to `/tmp/kiss-server-new` on EC2 → stop service → mv to `/usr/local/bin/kiss-server` → start service → verify `systemctl is-active` → create GitHub Release tagged `deploy/{sha}`
- Binary asset name: `kiss-server` (cd.yml line 58)
- GitHub Release: created by `softprops/action-gh-release@v2` (cd.yml line 54)

**Section 3 — How to check EC2 health:**
- SSH target: `ec2-user@54.83.192.65` (cd.yml line 41, CONTEXT.md)
- Service: `kiss-server` systemd unit
- Health commands: `sudo systemctl status kiss-server`, `sudo systemctl is-active kiss-server`
- Logs: `sudo journalctl -u kiss-server -n 50` (StandardOutput=journal in systemd unit per install-kiss-server.sh)

**Section 4 — Setup from scratch:**
High-level steps referencing scripts (D-05 — no inline script content):
1. Provision AWS infra → `scripts/setup-aws-infra.sh` (AWS CLI profile `kiss` required)
2. Set up EC2 service → `scripts/install-kiss-server.sh` (run on EC2 via SSH)
3. Configure iptables → `scripts/setup-iptables.sh` (run on EC2 via SSH)
4. Set up webroot → `scripts/setup-webroot.sh` (run on EC2 via SSH)
5. Configure DNS (GoDaddy A record → Elastic IP `54.83.192.65`)
6. Add GitHub secrets: `EC2_SSH_KEY` (private key content), `EC2_KNOWN_HOSTS` (EC2 host fingerprint)
7. Apply branch protection → `scripts/setup-branch-protection.sh` and `scripts/setup-prod-protection.sh`

### For DOCS-03 (README.md replacement)

**What it is:** From PROJECT.md: "A from-scratch HTTP/1.1 static file server written in pure Rust with no external dependencies beyond the `log` crate facade."

**Core value:** "A client can request any static file by path and receive a correct, RFC-compliant HTTP/1.1 response — without crashing, leaking filesystem paths, or serving the wrong content type." (REQUIREMENTS.md line 4)

**Build & run locally:**
```bash
cargo build --release
./target/release/kiss-server --root /path/to/webroot
```
Port defaults to 8080 (from systemd unit: `--port 8080`).

**Deployment:** Live at `http://ptodd.org/` on EC2 t3.micro (`54.83.192.65`). Deploy via: `git push origin origin/main:prod`

**Architecture summary (2-4 sentences for README):** Handler/Context/Router pattern with a fixed thread pool for concurrent connections. No external dependencies except the `log` crate facade. Static files served with binary-safe reads, MIME detection, and path traversal prevention.

### For scripts/README.md

All 8 scripts read and summarized:

| Script | Purpose | Run where | Prerequisites |
|--------|---------|-----------|---------------|
| `ci.sh` | Local CI check runner (fmt, clippy, build, test) | Developer machine | Rust toolchain installed locally |
| `install-kiss-server.sh` | Idempotent setup: swap, git, gcc, Rust, clone repo, build, install binary, systemd unit, enable+start service | EC2 (via SSH) | Fresh Amazon Linux 2023 EC2 instance |
| `setup-aws-infra.sh` | Provisions EC2 t3.micro, Security Group (ports 22+80), Elastic IP in us-east-1 | Developer machine | AWS CLI v2 configured with `kiss` profile |
| `setup-branch-protection.sh` | Creates/updates "Protect main" GitHub Ruleset (requires PR + CI status check) | Developer machine | `gh` CLI authenticated as ptdecker |
| `setup-iptables.sh` | Configures iptables: port 80 → 8080 redirect + INPUT ACCEPT rules, persists via iptables-services | EC2 (via SSH) | EC2 instance running; iptables-services installable via dnf |
| `setup-prod-protection.sh` | Creates/updates "Protect prod" GitHub Ruleset (deletion + non-fast-forward rules only) | Developer machine | `gh` CLI authenticated as ptdecker |
| `setup-webroot.sh` | Creates `/var/www/ptodd.org/` and writes Hello World `index.html` | EC2 (via SSH) | EC2 instance running |
| `verify-dns.sh` | Smoke test: A record, CNAME, HTTP content for ptodd.org and www.ptodd.org | Developer machine | `dig` and `curl` (both present on macOS) |

## Common Pitfalls

### Pitfall 1: Badge URL Derived from Workflow Name vs. File Name

**What goes wrong:** Badge URL uses the workflow display name (`name: CI`) rather than the workflow filename (`ci.yml`). They happen to match here, but the URL always uses the filename.

**Why it happens:** Conflating the `name:` key in the YAML with the filename. The badge URL path is `/actions/workflows/ci.yml/badge.svg` — it uses the filename, not the `name:` value.

**How to avoid:** Always derive the badge URL from the actual `.github/workflows/` filename. Here: `ci.yml` → URL contains `ci.yml`.

**Warning signs:** Badge renders as a broken image or shows a constant grey "unknown" state.

### Pitfall 2: docs/ Directory Does Not Exist Yet

**What goes wrong:** Writing docs/ci-cd.md or docs/design.md fails because the `docs/` directory has not been created.

**Why it happens:** The `docs/` directory is absent from the current repo (confirmed by directory check). Markdown file creation in a non-existent directory requires creating the parent first.

**How to avoid:** The Write tool creates parent directories automatically. No explicit `mkdir docs/` step is needed. Just write the files at the correct paths.

**Warning signs:** File write error referencing missing directory (only relevant if using shell commands rather than the Write tool).

### Pitfall 3: Deploy Command Format

**What goes wrong:** Documentation shows the deploy command as `git checkout prod && git merge main && git push origin prod`, which does not match how prod is actually promoted in this repo.

**Why it happens:** The typical "promote main to prod" pattern; but this repo uses the refspec shorthand.

**How to avoid:** The canonical deploy command (from STATE.md Phase 11 notes) is:
```bash
git push origin origin/main:prod
```
This pushes the current state of the remote `main` branch to `prod` without requiring a local checkout. Use this exact command in docs/ci-cd.md.

### Pitfall 4: Stale SSH Health Check Commands

**What goes wrong:** EC2 health check commands reference a non-existent user or wrong binary path.

**Why it happens:** Mixing up ec2-user (SSH login user) with kiss-server (the service user created by install-kiss-server.sh).

**How to avoid:** SSH as `ec2-user` (the login user). The service runs as the `kiss-server` system user internally. Binary is at `/usr/local/bin/kiss-server`. All `systemctl` commands require `sudo`.

## Environment Availability

Step 2.6: SKIPPED — this phase is docs-only. All four deliverables are Markdown files written to the local filesystem. No external tools, services, CLIs, databases, or network access are required at execution time.

## Validation Architecture

`nyquist_validation` is enabled in `.planning/config.json`.

### Test Framework

This phase has no automated test framework. The deliverables are documentation files (Markdown). Validation is structural and content inspection — verifiable by reading the output files and checking the live badge URL.

| Property | Value |
|----------|-------|
| Framework | None (documentation phase) |
| Config file | N/A |
| Quick run command | Manual inspection |
| Full suite command | Manual inspection |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DOCS-01 | README.md has CI badge before title, clickable link to CI runs | manual | `grep -n 'badge.svg' README.md` to confirm presence and placement | ❌ |
| DOCS-02 | `docs/ci-cd.md` exists with all four required sections | manual | `grep -n '^## ' docs/ci-cd.md` to confirm section headings | ❌ |
| DOCS-03 | README.md fully replaced with correct project description, build instructions, deployment overview | manual | `grep -n 'ptodd.org\|cargo build\|--root' README.md` to confirm key content | ❌ |

### Sampling Rate

- **Per task commit:** Visual inspection — open the file and confirm structure matches decision constraints
- **Per wave merge:** Review all four new files against the locked decisions (D-01 through D-10)
- **Phase gate:** All four files exist, badge renders correctly at `https://github.com/ptdecker/kiss-server/actions/workflows/ci.yml/badge.svg` before calling `/gsd:verify-work`

### Wave 0 Gaps

None — existing test infrastructure (none needed for documentation phase) covers all requirements. The phase gate validation is manual inspection of file content and live badge render.

## Code Examples

### Badge Markdown (DOCS-01)

```markdown
[![CI](https://github.com/ptdecker/kiss-server/actions/workflows/ci.yml/badge.svg)](https://github.com/ptdecker/kiss-server/actions/workflows/ci.yml)
```

This line is placed at line 1 of README.md, before any heading.

### Systemd Service Invocation (for docs/ci-cd.md EC2 health section)

```bash
# Check service status
ssh ec2-user@54.83.192.65 "sudo systemctl status kiss-server"

# Check if service is active (exits 0 if active)
ssh ec2-user@54.83.192.65 "sudo systemctl is-active kiss-server"

# Tail recent logs
ssh ec2-user@54.83.192.65 "sudo journalctl -u kiss-server -n 50"
```

Source: install-kiss-server.sh (`StandardOutput=journal` in systemd unit) + cd.yml verify step

### Deploy Command (for docs/ci-cd.md deploy section)

```bash
# Promote main to prod (triggers CD pipeline)
git push origin origin/main:prod
```

Source: STATE.md Phase 11 decision note

### Build & Run Locally (for README.md)

```bash
cargo build --release
./target/release/kiss-server --root /path/to/webroot --port 8080
```

Source: PROJECT.md + cd.yml build step

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Travis CI badges | GitHub Actions badges | ~2020 | Badge URL format changed; GitHub-native requires no third-party |
| shields.io for status badges | GitHub-native badge SVG | Project preference | Simpler, no external dependency |

## Open Questions

1. **Port in "Build & Run Locally" section**
   - What we know: The systemd unit uses `--port 8080`. The `--root` flag is documented as required. The `--port` flag default is not confirmed from a quick read of main.rs.
   - What's unclear: Whether `--port` has a default or is also required.
   - Recommendation: Document `--port 8080` explicitly in the build/run example. If port has a default, note it. This does not block the phase — the example can use `--port 8080` as the local development value regardless.

2. **docs/design.md Key Decisions table — full vs. curated**
   - What we know: Claude's discretion per CONTEXT.md. PROJECT.md has 13 decisions in the table.
   - What's unclear: Whether all 13 decisions add value for an external reader or whether implementation-detail decisions (Vec header storage, `write_to` consumes self) should be omitted from the public-facing doc.
   - Recommendation: Include all 13 from PROJECT.md. They are all legitimate design decisions that explain the codebase to a new reader. The table is compact and self-contained.

## Sources

### Primary (HIGH confidence)

- `.github/workflows/ci.yml` — CI trigger conditions, job name, toolchain, cache config, step sequence
- `.github/workflows/cd.yml` — CD trigger, build, deploy, health check, release creation steps
- `.planning/phases/12-badge-docs-readme/12-CONTEXT.md` — all locked decisions (D-01 through D-10), badge URLs, integration points
- `.planning/PROJECT.md` — architecture description, core value, key decisions table, constraints
- `scripts/*.sh` (all 8) — purpose, usage, prerequisites, execution context for each script
- `.planning/REQUIREMENTS.md` — DOCS-01, DOCS-02, DOCS-03 acceptance criteria
- `.planning/STATE.md` — Phase 11 decision: `git push origin origin/main:prod` as the canonical deploy command

### Secondary (MEDIUM confidence)

- GitHub Actions documentation (badge URL format is well-established; verified against CONTEXT.md confirmed URLs)

### Tertiary (LOW confidence)

- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — badge syntax is directly confirmed from CONTEXT.md; workflow files read directly
- Architecture: HIGH — all source material is in the repo; no external services; file structure is fully defined by locked decisions
- Pitfalls: HIGH — derived from reading actual source files and confirmed integration points

**Research date:** 2026-03-24
**Valid until:** Until ci.yml workflow filename changes or repo is renamed (stable indefinitely otherwise)
