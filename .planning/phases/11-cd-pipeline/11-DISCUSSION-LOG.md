# Phase 11: CD Pipeline - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-03-24
**Phase:** 11-cd-pipeline
**Areas discussed:** SSH credentials, Workflow file structure, prod branch workflow, Release asset naming

---

## SSH Credentials

| Option | Description | Selected |
|--------|-------------|----------|
| ~/.ssh/id_ed25519 | Same key used locally to SSH to ec2-user@54.83.192.65 | ✓ |
| New dedicated deploy key | Fresh ed25519 key pair specifically for CD — better least-privilege but requires EC2 authorized_keys update | |

**User's choice:** ~/.ssh/id_ed25519

| Option | Description | Selected |
|--------|-------------|----------|
| EC2_SSH_KEY | Clear, conventional name — ${{ secrets.EC2_SSH_KEY }} | ✓ |
| DEPLOY_SSH_KEY | More deployment-specific | |
| SSH_PRIVATE_KEY | Generic name | |

**User's choice:** EC2_SSH_KEY

---

## Workflow File Structure

| Option | Description | Selected |
|--------|-------------|----------|
| Separate cd.yml | New .github/workflows/cd.yml triggered on push to prod only | ✓ |
| Extend ci.yml | Add a deploy job to ci.yml, gated on branch == prod | |

**User's choice:** Separate cd.yml

| Option | Description | Selected |
|--------|-------------|----------|
| Deploy directly | prod is only pushed to after CI has passed on main — no re-running tests | ✓ |
| Run CI then deploy | Re-runs cargo build + test before deploying | |

**User's choice:** Deploy directly

---

## prod Branch Workflow

| Option | Description | Selected |
|--------|-------------|----------|
| Manual push: git push origin main:prod | Developer explicitly promotes main to prod | |
| PR merge into prod | Open a PR from main → prod to trigger deploy | ✓ |
| Tag-triggered | Push a git tag to trigger the workflow | |

**User's choice:** PR merge into prod

| Option | Description | Selected |
|--------|-------------|----------|
| No protection | prod is a deploy target — direct push is the intended workflow | |
| Restrict to admins only | Prevents accidental pushes from non-admins | ✓ |

**User's choice:** Restrict to admins only

---

## Release Asset Naming

| Option | Description | Selected |
|--------|-------------|----------|
| kiss-server | Simple, matches the binary name | ✓ |
| kiss-server-linux-amd64 | Includes platform info | |
| kiss-server-{sha} | Embeds commit SHA in asset name | |

**User's choice:** kiss-server

| Option | Description | Selected |
|--------|-------------|----------|
| deploy/{sha} | e.g. deploy/abc1234 — clearly marks deploy artifacts vs semver releases | ✓ |
| v{sha} or sha-{sha} | Alternative formats | |
| Just the SHA | Minimal, less descriptive | |

**User's choice:** deploy/{sha}

---

## Claude's Discretion

- Exact SSH known_hosts mechanism (ssh-keyscan vs static fingerprint)
- Whether to use appleboy/ssh-action or raw SSH in workflow steps
- Binary staging path on EC2 (e.g. /tmp/kiss-server-new)
- Prod branch protection script approach
- action-gh-release vs gh release create

## Deferred Ideas

None — discussion stayed within phase scope
