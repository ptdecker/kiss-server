---
phase: 25
plan: 04
subsystem: server/auth
tags: [rust, auth, middleware, jwt, security, rs256]
one_liner: "Production AuthMiddleware with RS256 JWT validation, per-vhost public-path exemptions, iss/aud tenant checks, and 302 redirect on auth failure"
dependency_graph:
  requires: [25-01, 25-02, 25-03]
  provides: [AuthMiddleware, VhostAuthConfig]
  affects: [src/server/auth.rs, src/server/mod.rs]
tech_stack:
  added: []
  patterns: [jwt-parse-verify-extract pipeline, per-vhost config map, SPKI DER key construction in tests]
key_files:
  created: []
  modified:
    - src/server/auth.rs
    - src/server/mod.rs
decisions:
  - "#[allow(dead_code)] on auth.rs module-level until Plan 05 wires main.rs (matches jwt/mod.rs pattern)"
  - "5 old X-Authenticated-User stub tests in mod.rs replaced with JWT-aware integration tests using build_integration_test_middleware/build_integration_test_jwt helpers"
  - "mod.rs integration test renamed: middleware_short_circuit_returns_401 -> middleware_short_circuit_returns_302_with_standard_headers (behavior changed)"
metrics:
  duration_minutes: 8
  completed_date: "2026-04-25"
  tasks_completed: 1
  tasks_total: 1
  files_modified: 2
---

# Phase 25 Plan 04: Auth Middleware JWT Pipeline Summary

Production-ready `AuthMiddleware` with full RS256 JWT validation pipeline, per-vhost auth policy, iss/aud claim validation, and 302 redirect on auth failure. Replaces the stub `X-Authenticated-User` header-trust implementation.

## What Was Built

`src/server/auth.rs` fully rewritten (~290 lines including tests):

- `VhostAuthConfig { login_url: String, public_paths: Vec<String> }` — per-vhost auth policy exposed for `main.rs` to populate
- `AuthMiddleware { spki_der, vhost_configs, issuer, audience }` — JWT-validating middleware with immutable fields (Send + Sync trivially satisfied)
- `AuthMiddleware::new(spki_der, vhost_configs, issuer, audience)` — constructor
- `Middleware::run` pipeline:
  1. Resolve vhost config — absent vhost returns Continue (ACFG-03)
  2. Check `ctx.decoded_path` against `public_paths` — match returns Continue (AMID-03)
  3. Extract Bearer token from `Authorization` header — missing or non-Bearer returns 302
  4. `jwt::parse` → `jwt::verify` → `jwt::extract` — any failure returns 302
  5. Validate `claims.iss == self.issuer` and `claims.aud == self.audience` — mismatch returns 302
  6. `ctx.auth = Some(AuthClaims { user_id: claims.sub })` — Continue (AMID-04)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Updated mod.rs integration tests broken by new AuthMiddleware constructor**

- **Found during:** Task 1 compile
- **Issue:** Five integration tests in `src/server/mod.rs` used `AuthMiddleware::new()` (no-arg stub API); the new 4-arg constructor caused compile errors
- **Fix:** Added `build_integration_test_middleware()` and `build_integration_test_jwt()` helpers in the mod.rs test block; updated all five tests to use the new API. Tests that previously sent `X-Authenticated-User: alice` now send a valid Bearer JWT. Tests that previously expected 401 now correctly expect 302 (reflecting the new redirect behavior). `middleware_short_circuit_returns_401_with_standard_headers` renamed to `middleware_short_circuit_returns_302_with_standard_headers`
- **Files modified:** `src/server/mod.rs`
- **Commit:** b7151dd (same commit — part of the same task)

**2. [Rule 2 - Missing critical functionality] Added #[allow(dead_code)] module attribute**

- **Found during:** Task 1 lint check
- **Issue:** Three dead_code warnings for `VhostAuthConfig`, `AuthMiddleware`, and `AuthMiddleware::new` — `main.rs` not yet wired (that's Plan 05)
- **Fix:** Added `#![allow(dead_code)]` at module level with explanatory comment, matching the identical pattern in `src/jwt/mod.rs`
- **Files modified:** `src/server/auth.rs`
- **Commit:** b7151dd

## Test Results

- `cargo test auth` — 15 tests passed (14 new auth.rs tests + 1 pre-existing `test_context_with_headers_injects_headers` matching filter)
- `cargo test` (full suite) — 292 tests passed, 0 failed
- `just lint` — 0 warnings

## Acceptance Criteria Verification

| Criterion | Status |
|-----------|--------|
| `cargo test auth` exits 0 with 14+ tests | PASS (15 tests) |
| `cargo test` full suite exits 0 | PASS (292 tests) |
| `just lint` produces 0 warnings | PASS |
| `pub struct VhostAuthConfig` present | PASS |
| `pub login_url: String` present | PASS |
| `pub public_paths: Vec<String>` present | PASS |
| `pub struct AuthMiddleware` present | PASS |
| `pub fn new(` present | PASS |
| `spki_der: Vec<u8>` struct field | PASS |
| `issuer: String` present | PASS |
| `audience: String` present | PASS |
| `jwt::parse` called | PASS |
| `jwt::verify` called | PASS |
| `jwt::extract` called | PASS |
| `ctx.auth = Some(AuthClaims` present | PASS |
| `claims.iss != self.issuer` present | PASS |
| `claims.aud != self.audience` present | PASS |
| `Response::new(302` present | PASS |
| No X-Authenticated-User / x-authenticated-user | PASS |
| No WWW-Authenticate | PASS |
| `git diff src/server/middleware.rs` empty | PASS (AMID-05) |

## Security Coverage (Threat Model)

All T-25-04-* threats addressed:
- T-25-04-01: alg confusion — `jwt::verify` enforces RS256 unconditionally (Phase 24 D-04)
- T-25-04-02: wrong-tenant token — `claims.iss != self.issuer` → redirect; tested by `auth_protected_path_wrong_issuer_redirects`
- T-25-04-03: wrong-application token — `claims.aud != self.audience` → redirect; tested by `auth_protected_path_wrong_audience_redirects`
- T-25-04-04: signature forgery — `jwt::verify` via ring; tested by `auth_protected_path_tampered_signature_redirects`
- T-25-04-05: path traversal — middleware uses `ctx.decoded_path` (percent-decoded by MiddlewareChain)
- T-25-04-06: expired token — `jwt::extract` validates exp; tested by `auth_protected_path_expired_token_redirects`
- T-25-04-07: failure oracle — all failure paths return identical 302 via `self.redirect()`
- T-25-04-08: header case sensitivity — `ctx.request.header("authorization")` is case-insensitive
- T-25-04-09: empty Bearer token — `strip_prefix("Bearer ").filter(!is_empty)` rejects; tested by `auth_bearer_with_no_token_redirects`
- T-25-04-11: ctx.auth populated on partial token — `ctx.auth = Some(...)` is last line; iss/aud checks happen before; tested by `auth_protected_path_wrong_issuer_redirects` and `auth_protected_path_wrong_audience_redirects`

## Known Stubs

None — plan goal achieved. `AuthMiddleware` is fully implemented. Wiring into `main.rs` is deferred to Plan 05.

## Self-Check: PASSED

- FOUND: `.planning/phases/25-auth-wiring/25-04-SUMMARY.md`
- FOUND: commit `b7151dd`
- FOUND: `src/server/auth.rs`
