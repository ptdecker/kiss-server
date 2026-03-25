# Phase 12: Badge, Docs, README - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-03-24
**Phase:** 12-badge-docs-readme
**Areas discussed:** CI Badge placement, docs/ci-cd.md depth, README rewrite scope

---

## CI Badge Placement

| Option | Description | Selected |
|--------|-------------|----------|
| Top of README, before title | Badge sits on its own line right above the h1 title — GitHub renders it prominently, standard OSS convention | ✓ |
| After the title/description | Badge appears below the project name and tagline | |
| Inline with the title | Badge on the same line as the # title heading | |

**User's choice:** Top of README, before title
**Notes:** Badge should be a clickable link to the GitHub Actions CI runs page (confirmed in follow-up).

---

## docs/ci-cd.md Depth

| Option | Description | Selected |
|--------|-------------|----------|
| Future-me (operator reference) | You — needs a reference when returning after months away | |
| External contributor onboarding | Someone unfamiliar with the setup | |
| Both equally | Covers onboarding + operator reference in one doc | ✓ |

**User's choice:** Both equally — dual-purpose doc

**Sections selected (all four):**
- How CI works + triggers
- How to deploy via prod branch
- How to check EC2 health
- Setup from scratch

**Setup from scratch depth:**

| Option | Description | Selected |
|--------|-------------|----------|
| High-level steps + script references | List steps with references to scripts | ✓ |
| Fully self-contained | Include all commands inline | |

---

## README Rewrite Scope

| Option | Description | Selected |
|--------|-------------|----------|
| Full replacement | Start fresh with a clean structure | ✓ |
| Targeted update | Keep error handling section, update setup instructions | |

**Sections selected:** All four proposed sections, plus two user-added items:
- What it is + core value
- Build & run locally
- Deployment overview
- Architecture / design notes
- **scripts/README.md** (user added): new file describing each script, referenced from top-level README
- **docs/design.md** (user added): detailed architecture walkthrough; README has brief summary + link

**Architecture depth:**

| Option | Description | Selected |
|--------|-------------|----------|
| Brief overview in README | 2-4 sentences, points to source | |
| Detailed walkthrough | Full design notes doc | |
| Brief in README + separate docs/design.md | Short summary + link in README; full walkthrough in docs/design.md | ✓ |

**User's notes:** "I'd like a detailed walkthrough in a separate design notes markdown file then in the top-level README.md have a short summary and link to the detailed walkthrough"

---

## Claude's Discretion

- Exact badge markdown syntax
- Exact heading structure and prose within each doc
- Whether docs/design.md includes the full key decisions table or a curated subset
- Tone and length within the section constraints

## Deferred Ideas

None
