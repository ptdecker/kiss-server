# Phase 4: URL Path Safety - Context

**Gathered:** 2026-03-02
**Status:** Ready for planning

<domain>
## Phase Boundary

Add `path()`, `decoded_path()`, and `query()` methods to the `Url` type; implement `is_safe()` for structural dot-dot rejection; have the `Router` automatically enforce `is_safe()` before dispatch. The `canonicalize()` + `starts_with(root)` traversal check (PATH-03) requires a known server root and belongs in `StaticFileHandler` (Phase 5) — the planner should note this requirement handoff.

Creating, modifying, or serving static files is a separate phase.

</domain>

<decisions>
## Implementation Decisions

### Status codes for rejected paths
- All path safety violations return **404 Not Found** — traversal (`..`), canonicalization escapes, and invalid percent-sequences alike
- No distinction between "not found" and "rejected for safety" — consistent, no information leakage to attackers

### Query string handling
- `path()` strips the query string and returns only the path component (e.g., `/file.html?v=1` → `/file.html`)
- `query()` returns `Option<&str>` — `None` when no query string is present, `Some(...)` with the raw query string when present
- Both methods added to `Url`; query strings are dropped before routing and traversal checks

### Where safety checks live
- `Url` provides `is_safe()` — checks for literal `..` path components (structural check, no filesystem needed)
- **Router automatically calls `url.is_safe()` before dispatch** — returns 404 if unsafe, no handler can forget the check
- `canonicalize()` + `starts_with(root)` check is deferred to Phase 5's `StaticFileHandler` (needs the configured root path)

### Claude's Discretion
- Exact signature of `is_safe()` (method vs associated function, return type)
- Whether `decoded_path()` returns `Result<String>` or `String` with fallback (given that invalid %-sequences return 404, a `Result` that the caller maps to 404 is idiomatic)
- Internal implementation of query string splitting (split on first `?`)

</decisions>

<specifics>
## Specific Ideas

- The existing `pct_decode` helper in `src/url/mod.rs` is already implemented and tested — `decoded_path()` should build on it rather than re-implementing decoding
- `is_safe()` should check decoded path components, not raw (so `%2E%2E` is also rejected)

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `pct_decode()` in `src/url/mod.rs`: Already implemented and tested, decodes a single percent-encoded character. `decoded_path()` will use this to decode the full path.
- `pct_encode()` in `src/url/mod.rs`: Exists but unlikely needed in this phase.
- `Router::dispatch()` in `src/server/router.rs`: The method to update — add `is_safe()` check before looking up handlers.

### Established Patterns
- All safety violations return 404 (matches Phase 1 pattern of returning errors rather than panicking)
- `Url` currently stores only `raw_path: String` — new methods will derive from this field
- Error handling uses `Result<T>` throughout; `decoded_path()` returning `Result<String>` fits existing patterns
- `#![allow(unused)]` is currently on `url/mod.rs` — this goes away once `path()` / `decoded_path()` are wired in

### Integration Points
- `Router::dispatch(&self, ctx: &mut Context)` — add `is_safe()` call at the top before handler lookup
- `Context.request.target: Url` — handlers access the URL via this field; `path()` and `decoded_path()` will be called on it
- `StaticFileHandler` (Phase 5) — will call `canonicalize()` using `decoded_path()` output + configured root

</code_context>

<deferred>
## Deferred Ideas

- `canonicalize()` + `starts_with(root)` traversal check — requires server root, deferred to Phase 5 StaticFileHandler (PATH-03)
- Fragment handling (`#anchor`) — not needed for server-side routing, not discussed

</deferred>

---

*Phase: 04-url-path-safety*
*Context gathered: 2026-03-02*
