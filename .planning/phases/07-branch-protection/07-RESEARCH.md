# Phase 7: Branch Protection - Research

**Researched:** 2026-03-11
**Domain:** GitHub Repository Rulesets API, `gh` CLI
**Confidence:** HIGH

## Summary

Phase 7 is a pure GitHub configuration phase — no Rust code changes. The goal is to create a branch ruleset on `ptdecker/kiss-server` that blocks direct pushes to `main` and requires the `ci` status check to pass before any PR can be merged. The repo owner (ptdecker, user ID 537053) must retain an emergency bypass so the solo developer is never locked out.

GitHub now recommends Rulesets over classic branch protection. The REST API endpoint `POST /repos/{owner}/{repo}/rulesets` accepts a fully declarative JSON body covering target branches, rules, and bypass actors in a single call. This is directly callable via `gh api --input`. The `ci` job name and its GitHub Actions app ID (15368) are already confirmed in the repo's check registry from Phase 6.

The implementation is three steps: (1) disable `allow_rebase_merge` on the repo via `gh repo edit`, (2) POST the ruleset JSON, (3) verify by attempting a direct push to main and observing rejection. All steps are scriptable and idempotent.

**Primary recommendation:** Use `gh api --method POST --input ruleset.json /repos/ptdecker/kiss-server/rulesets` from a shell script; the full JSON body is known and verified against the live repo.

---

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Configuration method**
- Create `scripts/setup-branch-protection.sh` alongside `scripts/ci.sh` from Phase 6
- Script uses the GitHub Rulesets API (`gh api /repos/{owner}/{repo}/rulesets`)
- Script is idempotent — can be re-run safely
- The plan includes a task to run the script against the real GitHub repo during execution (protection must be live, not just scripted)

**Required status checks**
- Require the `ci` job to pass before merge (name locked from Phase 6 context)
- Strict mode enabled: branch must be up-to-date with `main` before merging
- Update branch strategy is merge (not rebase) — consistent with repo merge-only preference
- No required reviewers — solo developer, CI check is the only gate

**Merge strategy (repo settings)**
- Enable "Allow merge commits" in GitHub repo settings
- Disable "Allow rebase merging" in GitHub repo settings
- This ensures GitHub's "Update branch" button always does a merge, not a rebase

**Protection mechanism**
- Use GitHub Rulesets (newer API), not classic branch protection rules
- Bypass actors: add self (repo owner account) explicitly so direct pushes remain possible in emergencies
- Bypass level: "always" — satisfies BRANCH requirement that admin bypass is not disabled

### Claude's Discretion

None — all decisions are locked.

### Deferred Ideas (OUT OF SCOPE)

None — discussion stayed within phase scope.
</user_constraints>

---

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| BRANCH-01 | Developer cannot push directly to `main` — all changes must go through a PR | Ruleset `non_fast_forward` + `deletion` rules block direct pushes; bypass actor preserves emergency access |
| BRANCH-02 | A PR cannot be merged to `main` unless the CI status check has passed | Ruleset `required_status_checks` rule with `context: "ci"` and `integration_id: 15368` (confirmed live); `strict_required_status_checks_policy: true` ensures branch must be up-to-date |
</phase_requirements>

---

## Standard Stack

### Core
| Tool | Version | Purpose | Why Standard |
|------|---------|---------|--------------|
| `gh` CLI | already installed | GitHub API calls, repo settings, verification | Used throughout the project; avoids curl + token management |
| GitHub Rulesets API | `2022-11-28` | Create branch protection rules | Replaces classic branch protection; single atomic API call |

### Supporting
| Tool | Version | Purpose | When to Use |
|------|---------|---------|-------------|
| `gh repo edit` | N/A | Set merge strategy (disable rebase) | One-time repo setting change |
| `gh api --input` | N/A | POST JSON body from file to GitHub API | Cleaner than `-f` flags for complex nested JSON |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Rulesets API | Classic branch protection (`/repos/{owner}/{repo}/branches/{branch}/protection`) | Classic is deprecated path; Rulesets are the current standard and support bypass actors more cleanly |
| `gh api --input` | `curl` with Authorization header | `gh` handles token automatically; no reason to use curl |

**Installation:**

```bash
# gh CLI already installed — no new dependencies
gh --version
```

---

## Architecture Patterns

### Recommended Project Structure

```
scripts/
├── ci.sh                       # Phase 6 — existing
└── setup-branch-protection.sh  # Phase 7 — new
```

### Pattern 1: Idempotent Ruleset Script

**What:** The script checks if a ruleset named "Protect main" already exists; if so, it PATCHes it. If not, it POSTs to create it. This makes the script safe to re-run.

**When to use:** Always — the CONTEXT.md explicitly requires idempotency.

**Detection approach:**
```bash
EXISTING_ID=$(gh api /repos/ptdecker/kiss-server/rulesets \
  | python3 -c "import sys,json; rules=json.load(sys.stdin); \
    match=[r['id'] for r in rules if r['name']=='Protect main']; \
    print(match[0] if match else '')")
```

If `$EXISTING_ID` is non-empty, use `PATCH /repos/ptdecker/kiss-server/rulesets/$EXISTING_ID`; otherwise use `POST /repos/ptdecker/kiss-server/rulesets`.

### Pattern 2: JSON Body via Temp File

**What:** Write the ruleset JSON to a temp file, pass it to `gh api --input`.

**When to use:** Always — the nested JSON structure is too complex for `-f` flags.

```bash
# Source: verified against live repo (check run for 'ci' job)
TMPFILE=$(mktemp /tmp/ruleset.XXXXXX.json)
cat > "$TMPFILE" << 'ENDJSON'
{
  "name": "Protect main",
  "target": "branch",
  "enforcement": "active",
  "conditions": {
    "ref_name": {
      "include": ["refs/heads/main"],
      "exclude": []
    }
  },
  "bypass_actors": [
    {
      "actor_type": "RepositoryRole",
      "actor_id": 5,
      "bypass_mode": "always"
    }
  ],
  "rules": [
    { "type": "deletion" },
    { "type": "non_fast_forward" },
    {
      "type": "required_status_checks",
      "parameters": {
        "required_status_checks": [
          {
            "context": "ci",
            "integration_id": 15368
          }
        ],
        "strict_required_status_checks_policy": true,
        "do_not_enforce_on_create": false
      }
    }
  ]
}
ENDJSON
gh api --method POST --input "$TMPFILE" /repos/ptdecker/kiss-server/rulesets
rm -f "$TMPFILE"
```

### Pattern 3: Merge Strategy via `gh repo edit`

**What:** Disable rebase merge so GitHub's "Update branch" button always does a merge commit.

**When to use:** Before creating the ruleset — merge commit is already enabled, only rebase needs disabling.

```bash
gh repo edit ptdecker/kiss-server \
  --enable-merge-commit \
  --enable-rebase-merge=false \
  --enable-squash-merge=false
```

**Current state (verified):**
- `allow_merge_commit: true` — already correct, no change needed
- `allow_rebase_merge: true` — must be disabled
- `allow_squash_merge: true` — disable for consistency with merge-only preference

### Anti-Patterns to Avoid

- **Using `-f` flags for nested JSON:** `gh api -f rules[0][type]=deletion` does not work for array-of-objects structures. Use `--input` with a temp file.
- **Hardcoding the ruleset ID:** The ruleset ID is assigned by GitHub at creation time. Always look it up dynamically for idempotency.
- **Omitting `integration_id` for required_status_checks:** Without `integration_id: 15368`, GitHub cannot distinguish the `ci` check from a status check of the same name set by an external service. Always include it.
- **Using classic branch protection:** The `/branches/{branch}/protection` endpoint is the old API. Rulesets are the current standard.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Checking if a ruleset exists | Custom existence logic | `gh api /repos/.../rulesets \| python3 -c "..."` | One-liner JSON parse; no complexity |
| Merge strategy settings | Manual GitHub UI steps | `gh repo edit` flags | Scriptable, repeatable, documented |
| Verifying protection is live | Custom git push test | `git push origin HEAD:main` and check exit code | Git's own error output confirms rejection |

**Key insight:** This entire phase is API calls and verification — no custom logic needed beyond a shell conditional for idempotency.

---

## Common Pitfalls

### Pitfall 1: Check Name Registry Dependency

**What goes wrong:** Trying to set `required_status_checks` before the `ci` job has ever run leaves GitHub unable to validate the check name. The ruleset is created but the check may silently not enforce.

**Why it happens:** GitHub's check registry only contains job names after they have reported at least once.

**How to avoid:** Phase 6 plan 02 confirmed CI run 22923908949 completed successfully. The `ci` job name is in the registry. This pitfall is already avoided.

**Warning signs:** If creating the ruleset on a fresh repo before any CI run, the `ci` check would need to be typed manually (not selected from a dropdown in the UI) — a hint it isn't registered yet.

### Pitfall 2: Wrong `actor_type` for Personal Repo

**What goes wrong:** Using `actor_type: "OrganizationAdmin"` on a personal (non-org) repo has no effect — organization roles don't apply to personal repos.

**Why it happens:** The GitHub docs list `OrganizationAdmin` as a valid actor type, but it only applies to org repos.

**How to avoid:** Use `actor_type: "RepositoryRole"` with `actor_id: 5` (admin role). This is the correct bypass for a personal repo owner.

**Warning signs:** If the ruleset is created without error but the repo owner still cannot push directly, the bypass actor is misconfigured.

### Pitfall 3: `strict_required_status_checks_policy` Without Understanding

**What goes wrong:** With `strict_required_status_checks_policy: true`, PRs whose branch is behind `main` cannot be merged — the "Merge" button is disabled until the branch is updated. This is intentional (CONTEXT.md decision) but can surprise the developer.

**Why it happens:** Strict mode prevents merging stale branches that might pass CI on an outdated codebase but fail after the merge.

**How to avoid:** The CONTEXT.md explicitly requires this; it is intentional. The developer must use the "Update branch" button (which will merge, not rebase, after repo settings are applied).

**Warning signs:** PR is stuck with "Branch is out of date" — this is the correct behavior, not a bug.

### Pitfall 4: Script Not Idempotent on Re-run

**What goes wrong:** Running the script twice creates a second ruleset with the same name. GitHub allows duplicate ruleset names.

**Why it happens:** POST always creates; there is no upsert endpoint.

**How to avoid:** Check for existing ruleset by name before POSTing; use PATCH if found.

---

## Code Examples

Verified patterns from live repo and GitHub API:

### Check Current Ruleset State
```bash
# Source: verified against ptdecker/kiss-server (returns [])
gh api /repos/ptdecker/kiss-server/rulesets
```

### Get GitHub Actions App ID for a Repo's Check Runs
```bash
# Source: verified against ptdecker/kiss-server — returns app_id: 15368
gh api /repos/ptdecker/kiss-server/commits/main/check-runs \
  | python3 -c "
import sys, json
d = json.load(sys.stdin)
for r in d.get('check_runs', []):
    print('name:', r.get('name'), 'app_id:', r.get('app', {}).get('id'))
"
```

### Disable Rebase Merge
```bash
# Source: gh repo edit --help (confirmed flags)
gh repo edit ptdecker/kiss-server \
  --enable-merge-commit \
  --enable-rebase-merge=false \
  --enable-squash-merge=false
```

### Verify Repo Merge Settings
```bash
gh api /repos/ptdecker/kiss-server \
  | python3 -c "
import sys, json
d = json.load(sys.stdin)
print('merge_commit:', d['allow_merge_commit'])
print('rebase_merge:', d['allow_rebase_merge'])
print('squash_merge:', d['allow_squash_merge'])
"
```

### Verify Direct Push is Blocked (after ruleset is live)
```bash
# Create a temp commit, attempt direct push to main — must be rejected
git commit --allow-empty -m "test: verify branch protection"
git push origin HEAD:main
# Expected: remote: error: GH006: Protected branch update failed
# Cleanup:
git reset --hard HEAD~1
```

### Verify Ruleset Was Created
```bash
gh api /repos/ptdecker/kiss-server/rulesets \
  | python3 -c "
import sys, json
rules = json.load(sys.stdin)
for r in rules:
    print('id:', r['id'], 'name:', r['name'], 'enforcement:', r['enforcement'])
"
```

---

## Key Facts (Verified Against Live Repo)

| Fact | Value | Source |
|------|-------|--------|
| GitHub owner | `ptdecker` | `gh api /user` |
| Repo name | `kiss-server` | `gh repo view` |
| CI job name | `ci` | Phase 6 SUMMARY; check run 22923908949 |
| GitHub Actions app ID | `15368` | `gh api /repos/ptdecker/kiss-server/commits/main/check-runs` |
| RepositoryRole admin actor_id | `5` | GitHub API docs + Terraform provider source |
| Current bypass actor type for personal repos | `RepositoryRole` | GitHub docs (OrganizationAdmin not applicable) |
| Current `allow_merge_commit` | `true` | `gh api /repos/ptdecker/kiss-server` |
| Current `allow_rebase_merge` | `true` (must disable) | `gh api /repos/ptdecker/kiss-server` |
| Current `allow_squash_merge` | `true` (disable for consistency) | `gh api /repos/ptdecker/kiss-server` |
| Existing rulesets | none | `gh api /repos/ptdecker/kiss-server/rulesets` returns `[]` |

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Classic branch protection (`/branches/{branch}/protection`) | Repository Rulesets (`/repos/{owner}/{repo}/rulesets`) | 2023 (GA) | Rulesets support bypass actors, multiple patterns, audit log; classic still works but is legacy path |
| Manual UI configuration | Scripted via `gh api` | Always available | Reproducible, version-controlled setup |

**Deprecated/outdated:**
- Classic branch protection API: Still functional but GitHub's UI now defaults to Rulesets. Don't use for new setups.

---

## Open Questions

1. **Squash merge: disable or leave enabled?**
   - What we know: CONTEXT.md says enable merge commits and disable rebase. Squash is not mentioned.
   - What's unclear: Should squash also be disabled for consistency? Current state is enabled.
   - Recommendation: Disable squash merge to enforce merge-only policy strictly. If squash is left on, a developer could accidentally squash instead of merge-commit.

---

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Manual verification via `gh` CLI + `git push` |
| Config file | None — behavioral test via GitHub API |
| Quick run command | `gh api /repos/ptdecker/kiss-server/rulesets` |
| Full suite command | `git push origin HEAD:main` (expect rejection) |

### Phase Requirements → Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| BRANCH-01 | Direct push to main is rejected | smoke | `git push origin HEAD:main` (exit non-zero = pass) | Wave 0 |
| BRANCH-02 | PR cannot merge without CI passing | manual | Open PR, verify merge button disabled until CI green | N/A (GitHub UI) |

### Sampling Rate

- **Per task commit:** `gh api /repos/ptdecker/kiss-server/rulesets` (verify ruleset exists with correct rules)
- **Per wave merge:** `git push origin HEAD:main` (verify push is rejected)
- **Phase gate:** Both BRANCH-01 and BRANCH-02 confirmed live before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] Direct push rejection test — create temp empty commit, push to main, verify non-zero exit; cleanup with `git reset --hard HEAD~1`

*(No test framework install needed — all verification is via gh CLI and git commands already available)*

---

## Sources

### Primary (HIGH confidence)

- Live `gh api /repos/ptdecker/kiss-server/commits/main/check-runs` — confirmed `ci` job name and GitHub Actions app ID 15368
- Live `gh api /repos/ptdecker/kiss-server` — confirmed current merge settings
- Live `gh api /repos/ptdecker/kiss-server/rulesets` — confirmed no existing rulesets
- Live `gh api /user` — confirmed owner login `ptdecker`, ID 537053
- `gh repo edit --help` — confirmed `--enable-rebase-merge=false` flag syntax

### Secondary (MEDIUM confidence)

- [GitHub Community Discussion #139808](https://github.com/orgs/community/discussions/139808) — complete working JSON body for Rulesets API, including `required_status_checks` with `integration_id`
- [GitHub REST API Docs: repos/rules](https://docs.github.com/en/rest/repos/rules) — endpoint reference, bypass_actors schema
- [GitHub Docs: Creating rulesets for a repository](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/creating-rulesets-for-a-repository) — bypass actors, admin eligibility

### Tertiary (LOW confidence — cross-verified with secondary)

- [GitHub rest-api-description issue #4406](https://github.com/github/rest-api-description/issues/4406) — community-discovered `actor_id` values (admin = 5); corroborated by Terraform provider source
- [Pulumi GitHub RepositoryRuleset docs](https://www.pulumi.com/registry/packages/github/api-docs/repositoryruleset/) — confirms `RepositoryRole` actor_id 5 for admin

---

## Metadata

**Confidence breakdown:**
- Ruleset JSON body: HIGH — community discussion shows working example; integration_id 15368 verified live
- Bypass actor config: HIGH — RepositoryRole actor_id 5 corroborated by multiple sources; OrganizationAdmin inapplicable to personal repos confirmed
- Merge strategy flags: HIGH — `gh repo edit --help` output verified
- Idempotency pattern: MEDIUM — standard shell pattern; not GitHub-specific risk

**Research date:** 2026-03-11
**Valid until:** 2026-06-11 (Rulesets API is stable; GitHub Actions app ID 15368 is a fixed platform constant)
