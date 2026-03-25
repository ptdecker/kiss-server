---
phase: 11
slug: cd-pipeline
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-24
---

# Phase 11 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | bash / shell verification + GitHub Actions live runs |
| **Config file** | `.github/workflows/cd.yml` |
| **Quick run command** | `bash -n .github/workflows/cd.yml && echo "Syntax OK"` |
| **Full suite command** | `git push origin main:prod` (triggers live CD run) |
| **Estimated runtime** | ~3-5 minutes (live run) |

---

## Sampling Rate

- **After every task commit:** Run `bash -n .github/workflows/cd.yml` (YAML/shell syntax check)
- **After every plan wave:** Inspect GitHub Actions tab for green run
- **Before `/gsd:verify-work`:** Full CD run must be green end-to-end
- **Max feedback latency:** 300 seconds (live GitHub Actions run)

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 11-01-01 | 01 | 0 | CD-01 | manual | `gh secret list` | ✅ | ⬜ pending |
| 11-01-02 | 01 | 0 | CD-01 | manual | `git branch -r \| grep prod` | ❌ W0 | ⬜ pending |
| 11-02-01 | 02 | 1 | CD-01,CD-02,CD-03,CD-04,CD-05 | live-run | `gh run list --workflow=cd.yml \| head -1` | ❌ W0 | ⬜ pending |

---

## Wave 0 Requirements

- [ ] `prod` branch must exist on remote before cd.yml workflow can trigger
- [ ] `EC2_SSH_KEY` secret must be set in GitHub repo
- [ ] `EC2_KNOWN_HOSTS` secret must be set in GitHub repo
- [ ] `prod` branch protection (admin-only) must be configured after cd.yml merges

*These are setup prerequisites — not test stubs. All CD requirements are verified by a live GitHub Actions run.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| CD workflow triggers on push to prod | CD-01 | Requires live push to prod branch | Open PR main→prod, merge it, observe GitHub Actions tab |
| Binary deploys atomically on EC2 | CD-03 | Requires live SSH to EC2 post-deploy | `ssh ec2-user@54.83.192.65 'ls -la /usr/local/bin/kiss-server'` |
| Health check fails pipeline on bad deploy | CD-04 | Requires intentional failure test | Manual: break service, push, confirm pipeline fails |
| GitHub Release created with binary attached | CD-05 | Requires live GH Release creation | Check GitHub Releases page for `deploy/{sha}` tag with `kiss-server` asset |
| http://ptodd.org/ still serves Hello World after deploy | CD-03 (integration) | End-to-end live site check | `curl -fs http://ptodd.org/ \| grep "Hello World"` |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 300s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
