# Phase 7: Branch Protection - Context

**Gathered:** 2026-03-10
**Status:** Ready for planning

<domain>
## Phase Boundary

Configure GitHub branch protection on `main` so direct pushes are blocked and PRs cannot be merged until the CI check passes. No Rust code changes — this is a pure GitHub configuration phase.

Branch protection must come after Phase 6 CI has run at least once (so the `ci` check name appears in GitHub's registry before it can be selected as a required check).

</domain>

<decisions>
## Implementation Decisions

### Configuration method
- Create `scripts/setup-branch-protection.sh` alongside `scripts/ci.sh` from Phase 6
- Script uses the GitHub Rulesets API (`gh api /repos/{owner}/{repo}/rulesets`)
- Script is idempotent — can be re-run safely
- The plan includes a task to run the script against the real GitHub repo during execution (protection must be live, not just scripted)

### Required status checks
- Require the `ci` job to pass before merge (name locked from Phase 6 context)
- Strict mode enabled: branch must be up-to-date with `main` before merging
- Update branch strategy is merge (not rebase) — consistent with repo merge-only preference
- No required reviewers — solo developer, CI check is the only gate

### Merge strategy (repo settings)
- Enable "Allow merge commits" in GitHub repo settings
- Disable "Allow rebase merging" in GitHub repo settings
- This ensures GitHub's "Update branch" button always does a merge, not a rebase

### Protection mechanism
- Use GitHub Rulesets (newer API), not classic branch protection rules
- Bypass actors: add self (repo owner account) explicitly so direct pushes remain possible in emergencies
- Bypass level: "always" — satisfies BRANCH requirement that admin bypass is not disabled

</decisions>

<specifics>
## Specific Ideas

- User explicitly wants merge-to-update (not rebase-to-update) — "Update branch" button should merge main into the branch
- Ruleset bypass: explicit account-level bypass actor, not role-based, for clarity

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `scripts/ci.sh`: Existing CI script from Phase 6 — setup-branch-protection.sh follows the same scripts/ convention

### Established Patterns
- `scripts/` directory established in Phase 6 — automation scripts live here
- `gh` CLI is available (used in CI/CD workflow planning)

### Integration Points
- Phase 6 CI job name is `ci` — this is the required check name for branch protection
- Phase 11 CD will push to `prod` branch — branch protection only applies to `main`, so CD is unaffected
- Phase 12 docs/ci-cd.md will document the protection setup — this phase creates the script, Phase 12 documents it

</code_context>

<deferred>
## Deferred Ideas

- None — discussion stayed within phase scope

</deferred>

---

*Phase: 07-branch-protection*
*Context gathered: 2026-03-10*
