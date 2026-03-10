# Project Retrospective

*A living document updated after each milestone. Lessons feed forward into future planning.*

---

## Milestone: v1.0 — MVP

**Shipped:** 2026-03-10
**Phases:** 6 | **Plans:** 16 | **Timeline:** ~10 days (2026-02-28 → 2026-03-10)

### What Was Built

- Eliminated all crashes and unsafe code in the existing foundation (poison-safe mutex, panic-safe Drop, `?` propagation replacing `unwrap_unchecked`)
- RFC-compliant HTTP/1.1 Response builder with CRLF-correct serialization and all mandatory headers (Content-Type, Content-Length, Date, Connection: close)
- Handler/Context/Router dispatch pipeline — first-match registered routes, NotFoundHandler fallback, 500 error path
- URL safety layer — percent-decoded routing and dot-dot component rejection before handler dispatch
- StaticFileHandler — binary-safe `fs::read()` returning `Vec<u8>`, 10-type MIME detection, `canonicalize()` traversal guard, HEAD mode, `--root` required at startup
- Tech debt cleanup — removed dead APIs (`Url::is_safe()`, `Url::query()`), stale `#[allow(dead_code)]`, fixed `peer_addr()?` propagation hazard

### What Worked

- **Strict dependency order paid off.** Each phase left the server in a testable state. Safety fixes before new features meant no inherited bugs in higher phases.
- **TDD on algorithmic code.** Howard Hinnant `civil_from_days` and `hex_char_to_byte` were straightforward to verify precisely with unit tests — no surprises at integration.
- **`set_fallback()` builder pattern** cleanly decoupled static file serving from the router core. StaticFileHandler registered as a fallback in one line in `main.rs`.
- **Requiring `--root`** (no default `.`) prevented a class of accidental path exposure and made the server's intent explicit.
- **Gap closure cycle** (verify → identify gaps → plan gap closure → re-verify) caught the `--root` default silently returning `Ok(".")` before it shipped.

### What Was Inefficient

- **Phase 05.1 inserted late for tech debt.** Stale `#[allow(dead_code)]` annotations accumulated through Phases 3–5 as forward-looking placeholders. A lighter-touch convention (e.g., a `// TODO: remove` comment instead of `allow`) would have made them more visible.
- **UAT port number discrepancy.** Test commands in early UAT used port 7878 and 8080 instead of the actual server port (6502). A CONTEXT.md note about the default port would have prevented this.
- **Phase 04 PATH-03 traceability misrouted.** PATH-03 was initially mapped to Phase 4 in REQUIREMENTS.md traceability but belonged to Phase 5. Required a doc-fix plan (05.1-02) to clean up.

### Patterns Established

- **`Result<T>` at all boundaries, `unwrap_or_else` only at process edges.** Zero panics in production code paths is achievable and was maintained across the full milestone.
- **Date header injected by `handle_connection`, not handlers.** Cross-cutting server concerns belong at the server layer — handlers stay pure.
- **404 on path traversal, not 403.** Consistent with OWASP: don't leak whether the path exists outside root.
- **`write_to` consumes `Response` (takes `self`).** Rust ownership prevents accidental double-send at compile time.
- **Hardcoded default paths are bugs, not conveniences.** `--root` required at startup is the right call — it forces explicit configuration.

### Key Lessons

1. **Fix the foundation first.** Shipping safety fixes before features means all new code builds on a stable base. No phase inherited a crash from the previous.
2. **Gap closure is part of the process, not a failure.** The `--root` default issue was caught by verify-work UAT and closed with a targeted 1-task plan. The workflow handled it gracefully.
3. **Decimal phases work.** Phase 5.1 (tech debt) inserted cleanly between Phase 5 and milestone completion without disrupting the phase numbering or tracking.
4. **The `Handler` trait `fn handle(&self, ctx: &mut Context)` design held up.** In-place mutation of `Context.response` through the whole pipeline required no reconstruction. Middleware will fit naturally.
5. **`civil_from_days` is the right algorithm for O(1) date math.** Well-documented, constant-time, and easy to test with known epoch values.

### Cost Observations

- Model: sonnet throughout (executor + verifier)
- All phases used parallel wave execution where possible
- Notable: Gap closure (05-05) took 1 task, ~5 min — the overhead of a targeted fix plan is minimal vs. catching a gap post-ship

---

## Cross-Milestone Trends

### Process Evolution

| Milestone | Phases | Plans | Key Change |
|-----------|--------|-------|------------|
| v1.0 | 6 | 16 | First milestone — baseline established |

### Cumulative Quality

| Milestone | Tests | Zero-Dep Additions | Gaps Found |
|-----------|-------|--------------------|------------|
| v1.0 | 83 | 0 (stdlib only) | 1 (--root default, closed) |

### Top Lessons (Verified Across Milestones)

1. Fix foundation before building features — every crash in v1.0 pre-dates the new code
2. Explicit over implicit — `--root` required, no silent defaults
