---
phase: 21-plugin-infrastructure
plan: "03"
subsystem: planning
tags: [architecture, decisions, auth, plugins, documentation]
dependency_graph:
  requires: []
  provides: [v1.5-plugin-arch.md]
  affects: [21-01, 21-02, 22-middleware-infrastructure]
tech_stack:
  added: []
  patterns: []
key_files:
  created:
    - .planning/decisions/v1.5-plugin-arch.md
  modified: []
decisions:
  - "ARCH-01: Dynamic library loading rejected — no stable Rust ABI, libloading blocked by D-03, unsafe at every boundary, zero benefit for first-party plugins"
  - "ARCH-02: Trait object + startup registration selected as canonical plugin pattern — uses existing Handler trait, zero new crates, config-driven activation"
  - "AUTH-01: Auth placement as pre-dispatch middleware — cites CVE-2025-61928 and OpenClaw gateway bypass as evidence that plugin-route auth is a documented failure mode"
  - "AUTH-02: MVP auth uses managed service (Clerk) + Lambda@Edge JWT validation + X-Authenticated-User header — no crypto in Rust until D-03 crates are justified"
  - "AUTH-03: Post-MVP auth replacement swaps Lambda@Edge for Rust JwtMiddleware — triggers when serde/serde_json are justified by chat engine; AuthClaims struct and ctx.auth field unchanged"
metrics:
  duration_seconds: 142
  completed_date: "2026-04-15"
  tasks_completed: 1
  tasks_total: 1
  files_created: 1
  files_modified: 0
---

# Phase 21 Plan 03: v1.5 Plugin Architecture Decision Record Summary

**One-liner:** Five locked architecture decisions — dynamic library rejection (ABI instability + D-03), trait object registration pattern, auth-as-middleware with CVE evidence, Clerk+Lambda@Edge MVP auth, and post-MVP JWT migration path.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Write the architecture decision record | (see below) | .planning/decisions/v1.5-plugin-arch.md |

## Decisions Made

| ID | Decision |
|----|----------|
| ARCH-01 | Dynamic library loading rejected — four-researcher unanimous consensus. Primary: Rust has no stable ABI (`Box<dyn Handler>` across `.so` boundary is UB). Secondary: `libloading` is on crates.io, blocked by D-03. |
| ARCH-02 | Trait object + startup registration is canonical plugin pattern. Plugins implement `KissPlugin: Handler`. Stored as `Box<dyn Handler>` in Router. Registered via `Router::add_prefix()`. Config-driven via `[[plugin]]` TOML blocks. |
| AUTH-01 | Auth runs as pre-dispatch middleware, not plugin handler. Cites CVE-2025-61928 and OpenClaw gateway bypass — both are plugin-route auth bypasses caused by routes outside the auth-checked dispatch path. |
| AUTH-02 | MVP: Clerk (managed auth) + Lambda@Edge JWT validation + `X-Authenticated-User` header. Rust reads header, not JWTs. `jsonwebtoken` deferred until D-03 crates are justified by chat engine. |
| AUTH-03 | Post-MVP: swap Lambda@Edge for Rust `JwtMiddleware` when `serde`/`serde_json` justified. Migration is 5 steps. `AuthClaims`, `ctx.auth`, and middleware chain architecture are unchanged — only implementation swaps. |

## Output Artifact

**File:** `.planning/decisions/v1.5-plugin-arch.md`

**Structure:**
- ARCH-01: Dynamic Library Loading — Rejected (4 technical rationale points, alternatives table)
- ARCH-02: Trait Object + Startup Registration — Selected (pattern description, canonical sketch, future evolution to kiss-plugin-sdk)
- AUTH-01: Auth Placement — Middleware (option comparison table, CVE citations, implementation notes)
- AUTH-02: MVP Auth Strategy (5-step architecture, dependency justification, build-order implications)
- AUTH-03: Post-MVP Auth Replacement Path (trigger condition, dependency impact table, 5 migration steps, what does NOT change)

## Deviations from Plan

None — plan executed exactly as written. This was a documentation write-up task with fully determined content from prior research. All five sections were written per the plan's exact specifications.

## Threat Surface Scan

No security-relevant surface introduced. This plan produces a documentation artifact only (`.planning/decisions/v1.5-plugin-arch.md`). The file is gitignored and contains only architecture rationale — no secrets, no runtime code, no network endpoints.

## Known Stubs

None. This plan produces no UI components and no data flows — documentation only.

## Self-Check: PASSED

- `.planning/decisions/v1.5-plugin-arch.md` exists: FOUND
- `## ARCH-01` heading: FOUND
- `No stable Rust ABI`: FOUND
- `libloading`: FOUND
- `unsafe`: FOUND
- `## ARCH-02` heading with `KissPlugin: Handler` and `Box<dyn Handler>`: FOUND
- `## AUTH-01` heading with `CVE-2025-61928` and `OpenClaw`: FOUND
- `## AUTH-02` heading with `Lambda@Edge`, `X-Authenticated-User`, `Clerk`: FOUND
- `## AUTH-03` heading with `jsonwebtoken` and `serde`: FOUND
- `##` heading count >= 5: FOUND (5 section headings)
