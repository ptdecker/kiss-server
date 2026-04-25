---
phase: 25
plan: 02
subsystem: jwt
tags: [rust, jwt, der, spki, refactor, pub-crate]
dependency_graph:
  requires: []
  provides: [crate::jwt::wrap_rsa_pubkey_as_spki, crate::jwt::encode_der_length]
  affects: [src/jwt/mod.rs]
tech_stack:
  added: []
  patterns: [pub(crate) visibility promotion, defensive DER length encoding]
key_files:
  created: []
  modified:
    - src/jwt/mod.rs
decisions:
  - "Replaced panic! in encode_der_length with defensive 3-byte capping per T-25-02-01: server must not crash on oversized input"
  - "pub(crate) chosen over pub to keep helpers off the public crate surface per T-25-02-02"
metrics:
  duration: "~10 minutes"
  completed: "2026-04-25"
  tasks_completed: 1
  tasks_total: 1
  files_modified: 1
---

# Phase 25 Plan 02: Promote DER helpers to pub(crate) Summary

Moved `wrap_rsa_pubkey_as_spki` and `encode_der_length` from `#[cfg(test)]` to module level with `pub(crate)` visibility, replacing the `panic!` in `encode_der_length` with a defensive 3-byte length form.

## Tasks Completed

| Task | Name | Commit | Files |
|------|------|--------|-------|
| 1 | Promote wrap_rsa_pubkey_as_spki and encode_der_length to pub(crate) | 94f038c | src/jwt/mod.rs |

## Verification

- `cargo test` (full suite): 252 passed, 0 failed
- `just lint`: 0 warnings
- `pub(crate) fn wrap_rsa_pubkey_as_spki` exists at module level (line 327)
- `pub(crate) fn encode_der_length` exists at module level (line 357)
- Each function appears exactly once in the file (no duplicates)
- `panic!("test helper: length too large")` removed — replaced with 3-byte form
- 4 new `encode_der_length_*` tests added and passing

## Deviations from Plan

None — plan executed exactly as written.

## Known Stubs

None.

## Threat Flags

No new security surface introduced. Threat mitigations T-25-02-01 and T-25-02-02 applied as required:
- T-25-02-01: panic removed, defensive 3-byte capping used instead
- T-25-02-02: pub(crate) keeps helpers internal to the crate

## Self-Check: PASSED

- src/jwt/mod.rs: FOUND (modified)
- Commit 94f038c: FOUND
- `pub(crate) fn wrap_rsa_pubkey_as_spki` at line 327: FOUND
- `pub(crate) fn encode_der_length` at line 357: FOUND
- 252 tests passing: CONFIRMED
- 0 lint warnings: CONFIRMED
