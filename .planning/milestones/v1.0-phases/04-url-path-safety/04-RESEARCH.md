# Phase 4: URL Path Safety - Research

**Researched:** 2026-03-02
**Domain:** Rust std — percent-decoding, path component validation, router guard placement
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

#### Status codes for rejected paths
- All path safety violations return **404 Not Found** — traversal (`..`), canonicalization escapes, and invalid percent-sequences alike
- No distinction between "not found" and "rejected for safety" — consistent, no information leakage to attackers

#### Query string handling
- `path()` strips the query string and returns only the path component (e.g., `/file.html?v=1` → `/file.html`)
- `query()` returns `Option<&str>` — `None` when no query string is present, `Some(...)` with the raw query string when present
- Both methods added to `Url`; query strings are dropped before routing and traversal checks

#### Where safety checks live
- `Url` provides `is_safe()` — checks for literal `..` path components (structural check, no filesystem needed)
- **Router automatically calls `url.is_safe()` before dispatch** — returns 404 if unsafe, no handler can forget the check
- `canonicalize()` + `starts_with(root)` check is deferred to Phase 5's `StaticFileHandler` (needs the configured root path)

### Claude's Discretion
- Exact signature of `is_safe()` (method vs associated function, return type)
- Whether `decoded_path()` returns `Result<String>` or `String` with fallback (given that invalid %-sequences return 404, a `Result` that the caller maps to 404 is idiomatic)
- Internal implementation of query string splitting (split on first `?`)

### Deferred Ideas (OUT OF SCOPE)
- `canonicalize()` + `starts_with(root)` traversal check — requires server root, deferred to Phase 5 StaticFileHandler (PATH-03)
- Fragment handling (`#anchor`) — not needed for server-side routing, not discussed
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| PATH-01 | Server percent-decodes request paths before routing and file resolution | `decoded_path()` on `Url` using existing `pct_decode` helper; `Router::dispatch` routes on decoded path |
| PATH-02 | Server rejects paths with `..` components, returning 404 | `is_safe()` on `Url` checks decoded components; `Router::dispatch` calls it before handler lookup |
| PATH-03 | Server uses `canonicalize()` + `starts_with(root)` check to prevent path traversal, returning 404 | **Deferred to Phase 5** per locked decision — noted as requirement handoff |
</phase_requirements>

---

## Summary

Phase 4 adds URL path safety to the existing `Url` type and `Router`. The work is pure Rust std — no external crates are added. Three methods are added to `Url`: `path()` (strips query string), `query()` (returns the raw query string), and `decoded_path()` (percent-decodes `path()`, returning `Result<String>`). A fourth method, `is_safe()`, checks whether every path component is free of literal `..` (operating on the decoded path so that `%2E%2E` is also caught). The Router's `dispatch` method gains a safety guard at its entry point that short-circuits to 404 for any URL that fails `is_safe()`.

The existing `pct_decode` function in `src/url/mod.rs` already decodes a single percent-encoded sequence correctly for all Unicode planes and is tested. `decoded_path()` must walk the raw path character-by-character, calling `pct_decode` at each `%` and passing through ASCII characters unchanged. PATH-03 (`canonicalize` + `starts_with`) is explicitly out of scope for this phase and is handed off to Phase 5 as a note in the plan.

The two places that change are `src/url/mod.rs` (four new methods on `Url`, remove the `#![allow(unused)]` stub) and `src/server/router.rs` (one guard call at the top of `dispatch`). The `Router` needs to call `decoded_path()` to produce the safe decoded path — but per the locked decisions the `is_safe()` check happens on the decoded path components. The guard path — where `decoded_path()` returns `Err` (invalid %-sequence) or `is_safe()` returns `false` — must write a 404 directly and return `Ok(())` (not `Err`) because the existing 500-error path in `handle_connection` would otherwise override with 500.

**Primary recommendation:** Add `path()`, `query()`, `decoded_path() -> Result<String>`, and `is_safe() -> bool` to `Url`; add a two-line guard at the top of `Router::dispatch` that calls `decoded_path()` + `is_safe()` and short-circuits to 404 on failure. Remove the `#![allow(unused)]` lint suppressor once methods are used.

---

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Rust std | (edition 2021) | String splitting, char iteration, path component checking | No external crates; project constraint is zero new deps |

No new dependencies are added in this phase. All functionality uses existing std primitives.

**Installation:** No changes to `Cargo.toml`.

---

## Architecture Patterns

### File Locations

```
src/
├── url/
│   └── mod.rs      # Add: path(), query(), decoded_path(), is_safe(); remove #![allow(unused)]
└── server/
    └── router.rs   # Add: safety guard at top of dispatch()
```

### Pattern 1: Query String Splitting in `path()` and `query()`

**What:** Split `raw_path` on the first `?` character. Everything before is the path; everything after is the query string.

**When to use:** Any time the server needs the routable path segment or the query parameters separately.

**Example:**
```rust
// Source: RFC 3986 §3.4 — query is everything after the first '?'
impl Url {
    /// Returns the path component only, stripping any query string.
    /// "/file.html?v=1" => "/file.html"
    pub fn path(&self) -> &str {
        match self.raw_path.find('?') {
            Some(idx) => &self.raw_path[..idx],
            None => &self.raw_path,
        }
    }

    /// Returns the raw query string (without the leading '?'), or None.
    /// "/file.html?v=1" => Some("v=1")
    /// "/file.html"     => None
    pub fn query(&self) -> Option<&str> {
        self.raw_path.find('?').map(|idx| &self.raw_path[idx + 1..])
    }
}
```

### Pattern 2: Full-Path Percent Decoding in `decoded_path()`

**What:** Walk the path component character by character. At each `%`, collect the three-character sequence (`%HH`) and call the existing `pct_decode` helper. Concatenate decoded characters into a `String`. Return `Err` on any invalid sequence; the caller (Router guard) maps `Err` to 404.

**When to use:** Before any routing or filesystem operation that needs the real filename (PATH-01).

**Example:**
```rust
// Source: existing pct_decode helper in src/url/mod.rs
impl Url {
    /// Percent-decodes the path component.
    ///
    /// Returns Err if the path contains an invalid %-sequence.
    /// The caller must map Err to a 404 response — it signals "reject this request".
    pub fn decoded_path(&self) -> Result<String> {
        let raw = self.path();
        let mut out = String::with_capacity(raw.len());
        let mut chars = raw.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '%' {
                // Collect "%HH" — two hex digits after the %
                let h1 = chars.next().ok_or("invalid %-sequence: truncated")?;
                let h2 = chars.next().ok_or("invalid %-sequence: truncated")?;
                let seq = format!("%{h1}{h2}");
                let decoded_char = pct_decode(&seq)?;
                out.push(decoded_char);
            } else {
                out.push(c);
            }
        }
        Ok(out)
    }
}
```

Note: `pct_decode` already handles multi-byte UTF-8 sequences by consuming multiple `%HH` pairs. Review the existing implementation carefully — it reads all `%HH` pairs from the input until the string ends, not just one. The `decoded_path` walker must not consume bytes that belong to the next character. The current `pct_decode` signature takes a `&str` representing the entire sequence for one Unicode character (e.g., `"%C3%A9"` for `é`). This means the walker must identify how many `%HH` pairs form a single Unicode character before passing the slice to `pct_decode`. The simpler approach: decode byte-by-byte into a `Vec<u8>`, then convert the whole buffer with `String::from_utf8`, avoiding multi-pair per call complexity entirely.

### Pattern 2b: Byte-Buffer Decoding (Recommended over per-char approach)

**What:** Accumulate raw bytes into a `Vec<u8>`, then convert to `String` once. Avoids the multi-byte per-call complexity of `pct_decode`.

**Example:**
```rust
pub fn decoded_path(&self) -> Result<String> {
    let raw = self.path();
    let mut bytes: Vec<u8> = Vec::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            let h1 = chars.next().ok_or("invalid %-sequence: truncated")?;
            let h2 = chars.next().ok_or("invalid %-sequence: truncated")?;
            let hi = hex_char_to_byte(h1)?;
            let lo = hex_char_to_byte(h2)?;
            bytes.push((hi << 4) | lo);
        } else {
            // Push the UTF-8 encoding of the plain character
            let mut buf = [0u8; 4];
            bytes.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
        }
    }
    String::from_utf8(bytes).map_err(|e| e.to_string().into())
}
```

This calls the already-tested `hex_char_to_byte` helper directly (also private in `url/mod.rs`), assembles bytes, and uses `String::from_utf8` for the validity check. It correctly handles both ASCII and multi-byte UTF-8 characters in the path.

### Pattern 3: `is_safe()` — Structural Dot-Dot Check

**What:** Split the decoded path on `/`, check that no component equals `..`. The check operates on the decoded path so that `%2E%2E` (which decodes to `..`) is also caught.

**When to use:** After `decoded_path()` succeeds, before dispatching to any handler (PATH-02).

**Example:**
```rust
impl Url {
    /// Returns true if the decoded path contains no `..` components.
    /// Always call this on the *decoded* path to catch encoded variants like %2E%2E.
    pub fn is_safe(&self) -> bool {
        // If decoding fails, treat path as unsafe (caller handles the Err separately)
        match self.decoded_path() {
            Ok(decoded) => !decoded.split('/').any(|component| component == ".."),
            Err(_) => false,
        }
    }
}
```

Alternative: make `is_safe` take the decoded string as a parameter so the caller that already has `decoded_path()` doesn't pay for a second decode. This is the discretion area — the planner should pick whichever avoids double-decoding. A clean signature for the Router guard would be:

```rust
// Option A: is_safe() method that decodes internally (simple API, double-decodes)
if !ctx.request.target.is_safe() { /* 404 */ }

// Option B: Decode once, check components, route on decoded string (no double-decode)
let decoded = match ctx.request.target.decoded_path() {
    Ok(d) => d,
    Err(_) => { /* write 404, return Ok(()) */ return Ok(()); }
};
if decoded.split('/').any(|c| c == "..") { /* write 404, return Ok(()) */ return Ok(()); }
// route using decoded string...
```

Option B is preferred: it decodes once, avoids the double-decode of Option A, and keeps all 404 logic at the router guard site rather than spread across two methods.

### Pattern 4: Router Safety Guard

**What:** Add a guard at the very top of `Router::dispatch`, before any handler lookup. The guard calls `decoded_path()` and checks components. On failure, it writes a 404 response directly into `ctx.response` and returns `Ok(())`.

**Why `Ok(())` not `Err`:** The 500-error path in `handle_connection` is triggered by `Err` from `router.dispatch`. A 404 for a bad path must not become a 500. The handler already has the `NotFoundHandler` pattern to follow.

**Example:**
```rust
pub fn dispatch(&self, ctx: &mut Context) -> Result<()> {
    // Safety guard: reject unsafe paths with 404 before any handler sees the request
    let decoded = match ctx.request.target.decoded_path() {
        Ok(d) => d,
        Err(_) => return Self::not_found(ctx),
    };
    if decoded.split('/').any(|c| c == "..") {
        return Self::not_found(ctx);
    }

    let method = &ctx.request.method;
    // Route on decoded path (PATH-01: routing uses decoded path)
    for (route_method, route_path, handler) in &self.routes {
        if route_method == method && route_path.as_str() == decoded.as_str() {
            return handler.handle(ctx);
        }
    }
    NotFoundHandler.handle(ctx)
}

fn not_found(ctx: &mut Context) -> Result<()> {
    NotFoundHandler.handle(ctx)
}
```

Note: Routing comparison changes from `ctx.request.target.to_string()` (raw path) to the decoded path string. This is correct for PATH-01 — a request for `/my%20file.html` should match a route registered as `/my file.html`.

### Anti-Patterns to Avoid

- **Checking `..` on the raw (encoded) path only:** `%2E%2E` passes a raw check but decodes to `..`. Always check after decoding.
- **Returning `Err` from `dispatch` on path rejection:** The 500-error path in `handle_connection` fires on `Err`. Path rejection is a normal 404, not a server error — return `Ok(())` after writing the 404 response.
- **Leaving `#![allow(unused)]` in `url/mod.rs`:** Once `path()`, `decoded_path()`, and `is_safe()` are wired into the Router, all previously-unused items become used. Remove the lint suppressor as part of this phase.
- **Routing on `to_string()` (raw path) after adding decoded path support:** The Router currently compares `ctx.request.target.to_string()` (which returns `raw_path`) against route strings. After PATH-01, comparison must use the decoded path — otherwise `/my%20file.html` never matches `/my file.html`.
- **Calling `decoded_path()` multiple times inside one dispatch call:** Decoding involves allocation and iteration. Decode once, reuse the `String`.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Hex digit conversion | Custom hex parser | Existing `hex_char_to_byte()` in `url/mod.rs` | Already implemented and tested |
| Percent decoding | New decode logic | Existing `pct_decode()` + `hex_char_to_byte()` | Already handles all Unicode planes, tested |
| 404 response construction | Inline body/header building in guard | `NotFoundHandler.handle(ctx)` | Keeps 404 format consistent across the codebase |

**Key insight:** This phase extends existing, already-tested helpers — `hex_char_to_byte` and `pct_decode` are the correct building blocks. `decoded_path()` should call them directly rather than re-implementing hex parsing.

---

## Common Pitfalls

### Pitfall 1: Double-Encoding / Encoded Dot-Dot

**What goes wrong:** `is_safe()` checks the raw path for `..` but misses `%2E%2E`, `%2e%2e`, `%2E.`, `.%2E`, and mixed cases. All decode to `..`.
**Why it happens:** String comparison on the raw path before decoding.
**How to avoid:** Always run `is_safe()` on the decoded path, not on `raw_path`.
**Warning signs:** Test with `GET /%2E%2E/etc/passwd HTTP/1.1` — if it reaches a handler, the check is broken.

### Pitfall 2: Returning `Err` from Dispatch on Path Rejection

**What goes wrong:** The safety guard returns `Err(...)` instead of writing a 404 and returning `Ok(())`. The caller in `handle_connection` catches `Err` and calls `send_error_response` with 500, overwriting the correct 404.
**Why it happens:** Treating path rejection as an internal error rather than a normal HTTP condition.
**How to avoid:** Follow the `NotFoundHandler` pattern — write the 404 response into `ctx.response` and return `Ok(())`.
**Warning signs:** A test for traversal path returns HTTP 500 instead of 404.

### Pitfall 3: Routing Still Uses Raw Path After Decoding is Added

**What goes wrong:** `dispatch` calls `decoded_path()` for the safety check but then routes on `ctx.request.target.to_string()` (raw path). A request for `/my%20file.html` never matches a route for `/my file.html`.
**Why it happens:** Forgetting to change the routing comparison line after adding the decoded path variable.
**How to avoid:** Use the `decoded` variable for both the safety check and the route comparison.
**Warning signs:** Test for percent-encoded path of a registered route returns 404 instead of 200.

### Pitfall 4: `decoded_path()` Truncates Multi-Byte UTF-8 Characters

**What goes wrong:** A naive byte-at-a-time decode that uses `char` iteration from the input but `u8`-level output may mishandle non-ASCII characters in the non-encoded portion of the path (e.g., `/résumé.html`). If a non-ASCII char is pushed as `c as u8`, its multi-byte encoding is lost.
**Why it happens:** Confusing Rust's `char` (Unicode scalar value, 1–4 bytes in UTF-8) with a single byte.
**How to avoid:** Use `c.encode_utf8(&mut buf)` when pushing plain characters to the byte buffer, as shown in Pattern 2b above.
**Warning signs:** Paths with non-ASCII characters in the literal portion decode to garbled output.

### Pitfall 5: `#![allow(unused)]` Not Removed

**What goes wrong:** The lint suppressor on `url/mod.rs` hides future dead-code issues in that module.
**Why it happens:** Forgetting the cleanup step after the methods are wired in.
**How to avoid:** Remove `#![allow(unused)]` as part of this phase and confirm `cargo clippy` passes cleanly (the pre-commit hook runs `clippy -D warnings`).
**Warning signs:** `cargo clippy` suppressed warnings you can no longer see.

### Pitfall 6: `path()` Returns a `&str` Borrowed from `self` — Lifetime Tension

**What goes wrong:** `decoded_path()` calls `self.path()` to get a `&str`, then iterates it. This is fine. But if anyone tries to store `self.path()` and then call `self.decoded_path()` in the same expression, borrow checker will complain (immutable borrow while constructing `String`).
**Why it happens:** `path()` borrows `self.raw_path`; `decoded_path()` also borrows `self` to call `path()` internally.
**How to avoid:** Call `decoded_path()` first, then use its result. Don't interleave references to `path()` and owned return values.
**Warning signs:** Borrow checker error referencing multiple borrows of `self`.

---

## Code Examples

### Existing `hex_char_to_byte` and `pct_decode` (already in `src/url/mod.rs`)

```rust
// Source: src/url/mod.rs (existing, tested)
fn hex_char_to_byte(c: char) -> Result<u8> {
    match c {
        '0'..='9' => Ok(c as u8 - b'0'),
        'a'..='f' => Ok(c as u8 - b'a' + 10),
        'A'..='F' => Ok(c as u8 - b'A' + 10),
        _ => Err(format!("The character '{}' is not a valid hexadecimal digit.", c).into()),
    }
}
```

### `decoded_path()` — Byte-buffer approach (recommended)

```rust
// Source: Pattern 2b above — builds on existing hex_char_to_byte
pub fn decoded_path(&self) -> Result<String> {
    let raw = self.path();
    let mut bytes: Vec<u8> = Vec::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            let h1 = chars.next().ok_or("invalid %-sequence: truncated")?;
            let h2 = chars.next().ok_or("invalid %-sequence: truncated")?;
            let hi = hex_char_to_byte(h1)?;
            let lo = hex_char_to_byte(h2)?;
            bytes.push((hi << 4) | lo);
        } else {
            let mut buf = [0u8; 4];
            bytes.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
        }
    }
    String::from_utf8(bytes).map_err(|e| e.to_string().into())
}
```

### 404 Response (matches `NotFoundHandler` in `router.rs`)

```rust
// Source: src/server/router.rs NotFoundHandler (existing pattern)
let body = b"Not Found".to_vec();
let content_length = body.len().to_string();
ctx.response = Response::new(404, "Not Found")
    .header("Content-Type", "text/plain")
    .header("Content-Length", &content_length)
    .header("Connection", "close")
    .body(body);
Ok(())
```

### Router Guard (top of `dispatch`)

```rust
// Source: Pattern 4 above
pub fn dispatch(&self, ctx: &mut Context) -> Result<()> {
    let decoded = match ctx.request.target.decoded_path() {
        Ok(d) => d,
        Err(_) => return NotFoundHandler.handle(ctx),
    };
    if decoded.split('/').any(|c| c == "..") {
        return NotFoundHandler.handle(ctx);
    }

    let method = &ctx.request.method;
    for (route_method, route_path, handler) in &self.routes {
        if route_method == method && route_path.as_str() == decoded.as_str() {
            return handler.handle(ctx);
        }
    }
    NotFoundHandler.handle(ctx)
}
```

---

## State of the Art

| Old Approach | Current Approach | Impact |
|--------------|-----------------|--------|
| Route on raw path string (`to_string()`) | Route on decoded path (`decoded_path()`) | Enables PATH-01; percent-encoded paths now match routes correctly |
| No safety guard in Router | Safety guard at top of `dispatch` | PATH-02 enforced at the framework layer; no handler can accidentally bypass it |
| `#![allow(unused)]` suppressor on `url/mod.rs` | Removed once methods are wired | Clean lint output; Clippy warnings are enforced by pre-commit hook |

**Deferred to Phase 5 (not deprecated — still required):**
- `canonicalize()` + `starts_with(root)` check: PATH-03 requires a configured server root path, which exists only in `StaticFileHandler`. Phase 5 MUST implement this as the third defense layer.

---

## Open Questions

1. **Should `is_safe()` be a public method or only `decoded_path()` + inline component check?**
   - What we know: CONTEXT.md lists `is_safe()` as a planned method on `Url`; its exact signature is Claude's Discretion.
   - What's unclear: Whether a standalone `pub fn is_safe(&self) -> bool` method adds enough value to justify the API surface, given that the Router guard will already call it inline.
   - Recommendation: Expose `is_safe(&self) -> bool` as a public method on `Url` for testability (unit tests can verify it without going through the Router). Internally it calls `decoded_path()` and checks components.

2. **PATH-03 handoff — does the planner need a task for it?**
   - What we know: Phase 5 is explicitly responsible for PATH-03.
   - What's unclear: Whether Phase 4's plan should contain a zero-work "handoff note" task or simply a comment in PLAN.md.
   - Recommendation: Include a clearly-labeled note in the Phase 4 plan that PATH-03 is deferred to Phase 5, citing the decision. No implementation task needed in Phase 4.

3. **`Result<T>` type for `url/mod.rs` — is it `crate::Result<T>` or a local alias?**
   - What we know: `url/mod.rs` currently uses `use super::*;` which brings in the top-level `pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>`. The existing functions return `Result<u8>` and `Result<char>` with `String` errors converted via `.into()`. This works because `String: Into<Box<dyn Error>>`.
   - What's unclear: Nothing — the pattern is already established and working. `decoded_path()` returning `Result<String>` fits this existing type alias.
   - Recommendation: Continue using the `super::*`-imported `Result<T>` alias. No changes to error types needed.

---

## Sources

### Primary (HIGH confidence)
- Source code: `src/url/mod.rs` — existing `hex_char_to_byte`, `pct_decode`, `Url` struct; read directly
- Source code: `src/server/router.rs` — existing `dispatch`, `NotFoundHandler`; read directly
- Source code: `src/server/error.rs`, `src/server/mod.rs`, `src/main.rs` — Result type chain; read directly
- RFC 3986 §2.1 (percent-encoding), §3.3 (path component), §3.4 (query) — referenced in existing code comments

### Secondary (MEDIUM confidence)
- [Rust Path Traversal Guide](https://www.stackhawk.com/blog/rust-path-traversal-guide-example-and-prevention/) — confirms `canonicalize` + `starts_with` as the filesystem traversal check (Phase 5 concern)
- [OWASP Path Traversal](https://owasp.org/www-community/attacks/Path_Traversal) — confirms that `%2E%2E` and mixed encodings must be checked on the decoded form

### Tertiary (LOW confidence)
- None — all critical patterns verified from source code directly.

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — no new dependencies; all std primitives confirmed by reading the existing code
- Architecture: HIGH — patterns derived directly from the existing codebase (`pct_decode`, `NotFoundHandler`, `dispatch` structure)
- Pitfalls: HIGH — verified against both the code structure and known path traversal attack vectors

**Research date:** 2026-03-02
**Valid until:** 2026-04-02 (stable domain — Rust std does not change, patterns are structural)
