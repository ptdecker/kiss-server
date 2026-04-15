# kiss-server — Claude Code Project Instructions

## Branch Sync Protocol

**Run this at the start of every session and after completing any phase:**

```bash
git fetch origin
git checkout main && git pull origin main
git checkout prod && git merge main --no-edit
git checkout <working-branch>
git rebase main
git push origin <working-branch>   # --force-with-lease if rebase rewrote commits
```

**After every phase execution:**
- Push the working branch immediately: `git push origin <working-branch>`
- Clean up agent worktree branches: `git branch -D worktree-agent-*`
- Remove agent worktree directories: `git worktree prune`

**Force push rule:** Rebasing a working branch rewrites commits. After any rebase,
use `git push --force-with-lease` (not `--force`) to push the working branch.

## Repo Layout

- `src/` — Rust source
- `scripts/` — automation scripts (see `scripts/README.md`)
- `docs/` — design and CI/CD documentation
- `.github/workflows/` — CI and CD pipelines
- `.planning/` — GSD project management artifacts (local only, gitignored)

## Lint Policy

`just lint` must produce zero warnings. Clippy and rustc warnings are treated as errors and must be corrected before committing. If `just lint` emits any warning, fix it immediately.

## Testing

All changes must pass `cargo test` (88 tests) before committing.
Pre-commit hook runs the test suite automatically.

## Just Commands

Development:
- `just lint` — format and lint (`cargo fmt` + `cargo clippy`)
- `just build` — lint then build
- `just run` — lint, build, then run
- `just test` — lint, build, then test
- `just build-docs` — lint then generate rustdoc
- `just docs` — generate and open rustdoc in browser

Infrastructure:
- `just verify-dns` — run DNS smoke tests (`scripts/verify-dns.sh`)
- `just branch-protection` — apply main branch protection rules (`scripts/setup-branch-protection.sh`)
- `just prod-protection` — apply prod branch protection rules (`scripts/setup-prod-protection.sh`)

Deployment:
- `just deploy <VERSION>` — tag `vVERSION` and push to prod (e.g., `just deploy 1.2.0`)
