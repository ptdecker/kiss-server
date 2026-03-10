# Phase 6: CI Pipeline - Context

**Gathered:** 2026-03-10
**Status:** Ready for planning

<domain>
## Phase Boundary

GitHub Actions workflow that lints, builds, and tests on every push to main and every PR targeting main. Rust toolchain pinned in rust-toolchain.toml. Cargo cache in place for faster repeated runs. A local ci.sh script mirrors the CI steps exactly.

Branch protection (Phase 7), AWS infrastructure (Phase 8), and CD pipeline (Phase 11) are separate phases.

</domain>

<decisions>
## Implementation Decisions

### Trigger scope
- CI triggers on: push to `main`, pull_request targeting `main`
- No other branches trigger CI on push
- `prod` branch is CD-only (Phase 11 handles it separately)

### Workflow file
- Replace existing `.github/workflows/rust.yml` entirely
- New file: `.github/workflows/ci.yml`
- Job name: `ci` (Phase 7 branch protection will require this check by name)

### Job structure
- Single job, sequential steps: fmt → clippy → build → test
- Fail-fast: if fmt fails, subsequent steps are skipped
- Steps are NOT duplicated inline — the workflow calls `./scripts/ci.sh`

### Local CI script
- Create `scripts/ci.sh` as the single source of truth for CI steps
- Script runs: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo build`, `cargo test`
- GitHub Actions workflow calls `./scripts/ci.sh` — no step duplication
- Developers run `./scripts/ci.sh` locally to verify before pushing

### Clippy strictness
- `cargo clippy --all-targets -- -D warnings` — any warning fails CI
- `--all-targets` includes test targets (not just src/)
- `cargo test` always runs (codebase has unit tests in `#[test]` blocks)

### Toolchain pinning
- Create `rust-toolchain.toml` pinned to `1.93.1` (current local version)
- Components: `rustfmt`, `clippy`
- Targets: `x86_64-unknown-linux-gnu` — enables cross-compilation from macOS for local verification of Linux binary

### Claude's Discretion
- Cargo cache implementation (actions/cache configuration for registry + target dir)
- Exact `set -euo pipefail` and script header conventions
- Whether to add `--locked` to cargo commands (reasonable to add for reproducibility)

</decisions>

<specifics>
## Specific Ideas

- `scripts/ci.sh` pattern comes from user's other projects — familiar convention, not invented for this project
- The job must be named `ci` (not `build`) so Phase 7 can select it by name in GitHub's branch protection check registry

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `.github/workflows/rust.yml`: Existing basic workflow (build + test on main). Will be deleted and replaced by ci.yml.

### Established Patterns
- No `scripts/` directory exists yet — will be created
- No `rust-toolchain.toml` exists yet — will be created
- `Cargo.toml`: edition 2021, one dep (log 0.4.20). No workspace, simple single-crate project.
- Codebase has `#[test]` functions throughout — `cargo test` is not a no-op

### Integration Points
- Phase 7 branch protection reads the job name from this workflow's first CI run — must be `ci`
- Phase 11 CD will build `--release` on the same runner; this CI workflow establishes the cache infrastructure it will inherit

</code_context>

<deferred>
## Deferred Ideas

- None — discussion stayed within phase scope

</deferred>

---

*Phase: 06-ci-pipeline*
*Context gathered: 2026-03-10*
