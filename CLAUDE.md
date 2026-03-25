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
- `.planning/` — GSD project management artifacts

## Testing

All changes must pass `cargo test` (86 tests) before committing.
Pre-commit hook runs the test suite automatically.
