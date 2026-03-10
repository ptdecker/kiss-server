# Phase 6: CI Pipeline - Research

**Researched:** 2026-03-10
**Domain:** GitHub Actions, Rust CI toolchain, cargo caching
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- CI triggers on: push to `main`, pull_request targeting `main`; no other branches trigger CI on push; `prod` branch is CD-only
- Replace existing `.github/workflows/rust.yml` entirely; new file: `.github/workflows/ci.yml`; job name: `ci`
- Single job, sequential steps: fmt → clippy → build → test; fail-fast if fmt fails, subsequent steps skipped
- Steps are NOT duplicated inline — the workflow calls `./scripts/ci.sh`
- Create `scripts/ci.sh` as single source of truth: runs `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo build`, `cargo test`
- `cargo clippy --all-targets -- -D warnings` — any warning fails CI; `cargo test` always runs
- Create `rust-toolchain.toml` pinned to `1.93.1`; components: `rustfmt`, `clippy`; targets: `x86_64-unknown-linux-gnu`

### Claude's Discretion
- Cargo cache implementation (actions/cache configuration for registry + target dir)
- Exact `set -euo pipefail` and script header conventions
- Whether to add `--locked` to cargo commands (reasonable to add for reproducibility)

### Deferred Ideas (OUT OF SCOPE)
- None — discussion stayed within phase scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| CI-01 | Developer can see CI run on every push to `main` and milestone branches and on every PR targeting `main` | Workflow `on:` triggers; push to main + pull_request to main covers this |
| CI-02 | CI fails if `cargo fmt -- --check` reports formatting issues | `cargo fmt --check` in ci.sh; fail-fast sequential job |
| CI-03 | CI fails if `cargo clippy -- -D warnings` reports any lint warning | `cargo clippy --all-targets -- -D warnings` in ci.sh |
| CI-04 | CI fails if any `cargo test` test fails | `cargo test` in ci.sh; codebase has `#[test]` functions |
| CI-05 | Rust toolchain version pinned in `rust-toolchain.toml` | `rust-toolchain.toml` with channel = "1.93.1"; dtolnay/rust-toolchain auto-reads it |
| CI-06 | Cargo registry and build artifacts cached between CI runs | `Swatinem/rust-cache@v2` handles both paths automatically |
</phase_requirements>

---

## Summary

This phase creates a GitHub Actions CI workflow that runs on every push to `main` and every PR targeting `main`. The workflow has a single job named `ci` that calls `./scripts/ci.sh`, which runs fmt check, clippy, build, and test in sequence. A `rust-toolchain.toml` pins the toolchain to 1.93.1. Caching is handled by `Swatinem/rust-cache@v2`.

The existing `.github/workflows/rust.yml` is a basic auto-generated workflow (job named `build`, no fmt/clippy, uses `actions/checkout@v3`). It must be deleted and replaced by `ci.yml`. The new job name `ci` is architecturally significant: Phase 7 branch protection will require this exact name as the required status check.

The only design decision left to Claude's discretion is: (a) whether to add `--locked` to cargo commands in ci.sh, and (b) the exact caching action configuration. Research confirms `--locked` is best practice when `Cargo.lock` is committed (it is committed in this project). Research confirms `Swatinem/rust-cache@v2` is the ecosystem standard for Rust CI caching and handles both paths automatically.

**Primary recommendation:** Use `Swatinem/rust-cache@v2` for caching; add `--locked` to all cargo build/test commands in ci.sh; use `set -euo pipefail` in ci.sh header.

---

## Standard Stack

### Core
| Library/Action | Version | Purpose | Why Standard |
|----------------|---------|---------|--------------|
| `Swatinem/rust-cache` | v2 (latest: v2.8.2, Nov 2025) | Cache `~/.cargo` + `./target` | Purpose-built for Rust; handles key generation from Cargo.lock, rust version, toolchain file |
| `actions/checkout` | v4 | Checkout repo | Current standard; v3 was used in old rust.yml |
| `dtolnay/rust-toolchain` | `@master` or pinned SHA | Install toolchain from rust-toolchain.toml | Auto-reads rust-toolchain.toml when no explicit toolchain input provided |

### Supporting
| Library/Action | Version | Purpose | When to Use |
|----------------|---------|---------|-------------|
| `actions-rust-lang/setup-rust-toolchain` | v1 | Alternative to dtolnay | Adds problem matchers; more opinionated |
| `dsherret/rust-toolchain-file` | latest | Reads rust-toolchain.toml explicitly | When you want fully explicit file-based approach |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `Swatinem/rust-cache@v2` | `actions/cache` with manual paths | Swatinem auto-generates smart keys; manual approach requires careful path specification and key strategy |
| `dtolnay/rust-toolchain` | `actions-rust-lang/setup-rust-toolchain` | Both auto-read rust-toolchain.toml; dtolnay is lighter-weight |

**Installation (workflow step):**
```yaml
- uses: actions/checkout@v4
- uses: dtolnay/rust-toolchain@master   # reads rust-toolchain.toml automatically
- uses: Swatinem/rust-cache@v2
```

---

## Architecture Patterns

### Recommended Project Structure (new files this phase)
```
.github/
└── workflows/
    └── ci.yml                    # replaces rust.yml
scripts/
└── ci.sh                         # single source of truth for CI steps
rust-toolchain.toml               # toolchain pin at repo root
```

### Pattern 1: rust-toolchain.toml format
**What:** TOML file at repo root that rustup reads automatically; pins exact toolchain version plus components and cross-compile targets.
**When to use:** Always when you need reproducible toolchain across dev machines and CI.
**Example:**
```toml
# Source: https://rust-lang.github.io/rustup/overrides.html
[toolchain]
channel = "1.93.1"
components = ["rustfmt", "clippy"]
targets = ["x86_64-unknown-linux-gnu"]
```

**Notes:**
- `profile` field can be added (`minimal` reduces install time); omitting it uses rustup default
- `channel` accepts exact version (1.93.1), channel name (stable), or dated nightly
- `targets` is additive; the host target is always installed regardless

### Pattern 2: GitHub Actions workflow calling ci.sh
**What:** Workflow does environment setup only; all cargo commands live in ci.sh.
**When to use:** When local reproducibility matters — developers run the same script that CI runs.
**Example:**
```yaml
# .github/workflows/ci.yml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always

jobs:
  ci:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@master
      - uses: Swatinem/rust-cache@v2
      - name: Run CI
        run: ./scripts/ci.sh
```

### Pattern 3: ci.sh script structure
**What:** Shell script with strict error handling; runs fmt check, clippy, build, test in sequence.
**When to use:** Always — this is the locked decision.
**Example:**
```bash
#!/usr/bin/env bash
set -euo pipefail

cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo build --locked
cargo test --locked
```

**Notes on `--locked`:**
- `Cargo.lock` is committed in this project (not in `.gitignore`)
- `--locked` refuses to update `Cargo.lock` during build; fails if it's out of sync
- Best practice for CI when lockfile is committed (HIGH confidence per Cargo docs)
- Apply to `cargo build` and `cargo test`; not applicable to `cargo fmt` or `cargo clippy` (those don't build artifacts in the same way, but `--locked` on clippy is also valid and consistent)

### Pattern 4: Swatinem/rust-cache@v2 configuration
**What:** Drop-in cache action for Rust; caches `~/.cargo` (registry, git deps) and `./target` directory.
**When to use:** Always in Rust CI workflows.
**Example (default — no configuration needed):**
```yaml
- uses: Swatinem/rust-cache@v2
```
**Cache key components (automatic):** job ID, rustc version+hash, Cargo.toml/Cargo.lock hashes, rust-toolchain file hash, .cargo/config.toml hash.

**Optional tuning:**
```yaml
- uses: Swatinem/rust-cache@v2
  with:
    cache-on-failure: "true"    # save cache even when CI fails (speeds up fix iteration)
```

### Anti-Patterns to Avoid
- **Keeping `actions/checkout@v3`:** The old rust.yml uses v3. Use v4; v3 is outdated.
- **Job named `build`:** The old rust.yml has `jobs: build:`. Branch protection in Phase 7 requires `jobs: ci:` exactly. Do not reuse the old name.
- **Inline cargo commands in workflow steps:** The locked decision is to call ci.sh. Duplicating commands in the YAML breaks the "single source of truth" contract.
- **Running `cargo fmt` without `--check` in CI:** `cargo fmt` (without flag) modifies files and exits 0. `cargo fmt --check` exits non-zero on formatting issues — this is what CI needs.
- **Omitting `CARGO_TERM_COLOR: always`:** Without it, cargo output in Actions logs loses color, making errors harder to scan.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Cargo cache key generation | Custom `actions/cache` with manual key strategy | `Swatinem/rust-cache@v2` | Key must account for Rust version, OS, lockfile, toolchain file — Swatinem handles all of these correctly |
| Toolchain installation | Manual `rustup` install steps | `dtolnay/rust-toolchain@master` | Action handles rustup install, component installation, and target addition from rust-toolchain.toml in one step |
| Fail-fast sequential steps | Complex `if: success()` conditions | Default GitHub Actions behavior | Steps fail-fast by default in a single job — no extra configuration needed |

**Key insight:** For Rust CI, two actions (`dtolnay/rust-toolchain` + `Swatinem/rust-cache`) replace 20+ lines of manual setup/caching YAML.

---

## Common Pitfalls

### Pitfall 1: dtolnay/rust-toolchain and rust-toolchain.toml interaction
**What goes wrong:** Using `dtolnay/rust-toolchain@stable` (pinning the action to a channel) when you have `rust-toolchain.toml` causes the action to install `stable` rather than reading the file.
**Why it happens:** The `@rev` in the action name determines the toolchain when an explicit toolchain is provided; `rust-toolchain.toml` is only auto-read when no toolchain is specified to the action.
**How to avoid:** Use `dtolnay/rust-toolchain@master` with no `toolchain:` input. The action then reads `rust-toolchain.toml` and installs exactly what the file specifies.
**Warning signs:** CI uses a different toolchain version than local dev; `rustup show` in CI doesn't match `rust-toolchain.toml`.

### Pitfall 2: `ubuntu-latest` is now Ubuntu 24.04
**What goes wrong:** Code that relied on Ubuntu 22.04 packages or behavior may break.
**Why it happens:** GitHub migrated `ubuntu-latest` to Ubuntu 24.04 in January 2025.
**How to avoid:** For this project (pure Rust, no system deps), this is not a concern. If future phases need specific system packages, pin to `ubuntu-24.04` explicitly.
**Warning signs:** apt install failures for packages available in 22.04 but removed in 24.04.

### Pitfall 3: Swatinem/rust-cache and the target directory size
**What goes wrong:** Cache grows unbounded across runs; GitHub enforces a 10 GB total cache limit per repo and evicts oldest entries.
**Why it happens:** `target/` accumulates incremental artifacts from multiple configurations.
**How to avoid:** The action automatically excludes workspace crates from the cache. For a single-crate project with few deps, cache size is not a practical concern at this stage.
**Warning signs:** Cache restoration fails with "Cache Size limit exceeded"; builds get slower over time instead of faster.

### Pitfall 4: `cargo fmt --check` vs `cargo fmt`
**What goes wrong:** Running `cargo fmt` in CI modifies files in the ephemeral runner, exits 0, and CI never catches formatting issues.
**Why it happens:** Forgetting the `--check` flag; `cargo fmt` is the local fix command, `cargo fmt --check` is the CI verification command.
**How to avoid:** ci.sh uses `cargo fmt --check`. The Justfile uses `cargo fmt` (without check) — these serve different purposes and should not be conflated.
**Warning signs:** CI always passes even when code is unformatted.

### Pitfall 5: Cross-compilation on macOS requires a linker (local dev only)
**What goes wrong:** `cargo build --target x86_64-unknown-linux-gnu` on macOS fails with a linker error because macOS doesn't ship a Linux GCC linker.
**Why it happens:** The target `x86_64-unknown-linux-gnu` in `rust-toolchain.toml` enables local cross-compilation verification, but macOS needs an explicit cross-linker.
**How to avoid:** CI runs on `ubuntu-latest` where `x86_64-unknown-linux-gnu` is the native target — no linker needed. For local macOS cross-compilation:
  ```bash
  brew tap messense/macos-cross-toolchains
  brew install x86_64-unknown-linux-gnu
  # Then either set env var or add to .cargo/config.toml:
  # [target.x86_64-unknown-linux-gnu]
  # linker = "x86_64-unknown-linux-gnu-gcc"
  ```
  The ci.sh does NOT add `--target x86_64-unknown-linux-gnu` — it builds for the runner's native target. The `targets` entry in `rust-toolchain.toml` adds the cross-compile toolchain component without mandating its use in every build.
**Warning signs:** `error: linker 'x86_64-linux-gnu-gcc' not found` on macOS.

### Pitfall 6: Job name must be `ci` from the first CI run
**What goes wrong:** If `ci.yml` is merged but the job is named something other than `ci`, Phase 7 branch protection cannot select it as a required check until a run with the correct name exists.
**Why it happens:** GitHub's branch protection check registry is populated by actual CI runs, not by YAML parsing.
**How to avoid:** The `jobs:` key in `ci.yml` must be `ci:` exactly. This is already decided.
**Warning signs:** Phase 7 setup cannot find the check name in the dropdown.

---

## Code Examples

Verified patterns from official sources:

### rust-toolchain.toml (complete, ready to use)
```toml
# Source: https://rust-lang.github.io/rustup/overrides.html
[toolchain]
channel = "1.93.1"
components = ["rustfmt", "clippy"]
targets = ["x86_64-unknown-linux-gnu"]
```

### scripts/ci.sh (complete, ready to use)
```bash
#!/usr/bin/env bash
set -euo pipefail

cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo build --locked
cargo test --locked
```

### .github/workflows/ci.yml (complete, ready to use)
```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always

jobs:
  ci:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@master
      - uses: Swatinem/rust-cache@v2
      - name: Run CI
        run: ./scripts/ci.sh
```

**Note on file permissions:** `ci.sh` must be executable before committing. Run `chmod +x scripts/ci.sh` before `git add`.

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Manual `actions/cache` with hardcoded paths | `Swatinem/rust-cache@v2` | ~2021; v2 latest Nov 2025 | Smart key generation; no manual path configuration needed |
| `actions/checkout@v3` | `actions/checkout@v4` | Oct 2023 | v4 uses Node 20; v3 uses Node 16 (deprecated) |
| `ubuntu-latest` = Ubuntu 22.04 | `ubuntu-latest` = Ubuntu 24.04 | January 2025 | Pre-installed package list trimmed; affects projects with native deps |
| Committing `Cargo.lock` for binaries is optional/discouraged | Cargo team recommends committing for all packages | Aug 2023 (Rust blog) | `--locked` is now appropriate CI practice for committed lockfiles |

**Deprecated/outdated in existing rust.yml:**
- `actions/checkout@v3`: Replace with v4
- Job named `build`: Must be renamed to `ci` for Phase 7 compatibility
- No fmt/clippy steps: Addressed by this phase
- No caching: Addressed by this phase

---

## Open Questions

1. **Should `--locked` apply to `cargo clippy` as well?**
   - What we know: `--locked` works on clippy (it performs compilation); Cargo docs recommend `--locked` when lockfile is committed
   - What's unclear: Some teams apply it only to build/test; clippy with `--locked` is valid but less commonly documented
   - Recommendation: Apply `--locked` to both `cargo clippy` and `cargo build`/`cargo test` for maximum consistency. If it causes unexpected issues, remove from clippy only.

2. **`cache-on-failure: "true"` for Swatinem/rust-cache?**
   - What we know: Default is `false` (don't cache if CI fails); enabling it speeds up iteration when fixing CI failures
   - What's unclear: Whether it's worth the slight cache bloat for a project that rarely has CI failures
   - Recommendation: Enable it (`cache-on-failure: "true"`). During active development, CI failures happen and fast cache restoration is valuable.

---

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in `#[test]` (cargo test) |
| Config file | none — cargo discovers tests automatically |
| Quick run command | `cargo test` |
| Full suite command | `cargo test` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CI-01 | Workflow triggers on push/PR to main | manual | View GitHub Actions tab after push | N/A — GitHub-side verification |
| CI-02 | fmt failure blocks merge | manual | Introduce fmt error, open PR, verify red check | N/A — GitHub-side verification |
| CI-03 | clippy warning blocks merge | manual | Introduce clippy warning, open PR, verify red check | N/A — GitHub-side verification |
| CI-04 | test failure blocks merge | manual | Break a test, open PR, verify red check | N/A — GitHub-side verification |
| CI-05 | rust-toolchain.toml is read by CI | smoke | `cat rust-toolchain.toml` exists + CI log shows "1.93.1" | ❌ Wave 0: file to create |
| CI-06 | Cache restored on repeated runs | manual | Second CI run shows "Cache restored" in Actions log | N/A — GitHub-side verification |

**Note:** CI correctness is verified by observing GitHub Actions behavior, not by unit tests. The validation strategy is: (a) confirm files exist locally, (b) push and observe the Actions tab.

### Sampling Rate
- **Per task commit:** `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
- **Per wave merge:** same (single-wave phase)
- **Phase gate:** All 6 requirements verified via GitHub Actions observation before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `rust-toolchain.toml` — REQ CI-05 (create as part of Wave 1)
- [ ] `scripts/ci.sh` — REQ CI-02/03/04 (create as part of Wave 1, must be `chmod +x`)
- [ ] `.github/workflows/ci.yml` — REQ CI-01/02/03/04/06 (create as part of Wave 1, delete `rust.yml`)

*(No test framework gaps — cargo test is built-in and already works. All gaps are production files, not test infrastructure.)*

---

## Sources

### Primary (HIGH confidence)
- `https://rust-lang.github.io/rustup/overrides.html` — rust-toolchain.toml TOML format, all valid fields
- `https://github.com/Swatinem/rust-cache` — v2.8.2 (Nov 2025), configuration options, cached paths
- `https://doc.rust-lang.org/cargo/guide/cargo-toml-vs-cargo-lock.html` — Cargo.lock commit guidance, --locked recommendation
- `https://blog.rust-lang.org/2023/08/29/committing-lockfiles/` — Rust team's updated lockfile guidance (Aug 2023)

### Secondary (MEDIUM confidence)
- `https://github.blog/changelog/2024-09-25-actions-new-images-and-ubuntu-latest-changes/` — ubuntu-latest → Ubuntu 24.04 migration (completed Jan 2025)
- `https://github.com/dtolnay/rust-toolchain` — auto-reads rust-toolchain.toml when no toolchain input given
- `https://github.com/actions/runner-images/issues/10636` — Ubuntu 24.04 as ubuntu-latest confirmed

### Tertiary (LOW confidence)
- `https://github.com/messense/homebrew-macos-cross-toolchains` — macOS cross-compile linker setup for x86_64-unknown-linux-gnu (local dev only, not CI path)

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — Swatinem/rust-cache and dtolnay/rust-toolchain are verified against official repos; versions confirmed
- Architecture: HIGH — rust-toolchain.toml format verified against official rustup docs; workflow structure is straightforward
- Pitfalls: HIGH for fmt/clippy/job-name pitfalls (verified against docs); MEDIUM for macOS cross-compile linker (WebSearch, single source)

**Research date:** 2026-03-10
**Valid until:** 2026-06-10 (stable tooling; 90-day estimate)
