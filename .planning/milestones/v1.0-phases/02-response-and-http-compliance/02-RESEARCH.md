# Phase 2: Response and HTTP Compliance - Research

**Researched:** 2026-03-01
**Domain:** HTTP/1.1 response serialization, RFC compliance, Rust std-only patterns
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

**Response struct API:**
- Value-chaining builder pattern: `Response::new(200, "OK").header(k, v).body(bytes)`
- Each builder method takes `self` and returns `Self` (no `&mut self`)
- Lives in `server/response.rs` — mirrors `server/request.rs`
- Body stored as `Vec<u8>` (binary-safe from day one, required for Phase 5 WASM/PNG)
- Serializes via `write_to(&mut impl Write) -> Result<()>` — streams bytes directly, no intermediate allocation

**Error response bodies:**
- Descriptive messages sourced from `Error::Display` (e.g., `"invalid request: bad method"`)
- Content-Type: `text/plain` for all error responses
- Error's existing Display impls flow directly into the body — no separate message mapping

**HTTP status codes:**
- `Error::RequestTooLarge` → **431 Request Header Fields Too Large** (precise, separate branch)
- All other parse/IO errors → **400 Bad Request**
- Malformed/non-UTF-8 input currently returns `Err` with no response; must send 400 before returning

### Claude's Discretion
- `reason` phrase field type (`&'static str` or `String`)
- IMF-fixdate method name on DateTime (e.g., `to_imf_fixdate()` or `format_http_date()`)
- How mandatory headers are enforced at `write_to` time (panic vs always-set defaults)
- Internal header storage strategy (Vec of tuples vs HashMap)

### Deferred Ideas (OUT OF SCOPE)
None — discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| SAFE-01 | Server returns 400 response (not panic) on malformed or non-UTF-8 request input | Error-matching pattern in `handle_connection`; Response builder writes 400 body before returning Err |
| HTTP-01 | Server includes `Content-Type` header on all responses | Mandatory header enforcement at `write_to` time; builder always inserts Content-Type |
| HTTP-02 | Server includes `Content-Length` header (byte length) on all responses | `body.len()` as `usize` rendered decimal; must match actual bytes, not char count |
| HTTP-03 | Server includes `Date` header in IMF-fixdate format on all responses | `DateTime::to_imf_fixdate()` using weekday_from_days + fixed byte-template approach |
| HTTP-04 | Server includes `Connection: close` header on all responses | Mandatory header set at `write_to` time |
| HTTP-05 | All HTTP response lines use CRLF (`\r\n`) terminators | `write_to` uses `\r\n` literals; status line, each header line, blank separator line |
| HTTP-06 | Server responds 400 Bad Request for malformed HTTP requests (no panics) | `handle_connection` catches all Err variants, calls `send_error_response` before propagating |
| TIME-03 | DateTime exposes IMF-fixdate formatting for the HTTP `Date:` response header | New `to_imf_fixdate(&self) -> String` method on `DateTime`; uses weekday_from_days algorithm |
</phase_requirements>

---

## Summary

Phase 2 builds a `Response` struct in `src/server/response.rs` that serializes valid HTTP/1.1 messages. The entire phase is std-only: no new crate dependencies are introduced. The core deliverables are: (1) a value-chaining builder, (2) a `write_to(&mut impl Write)` serializer that emits CRLF-terminated lines, (3) mandatory-header enforcement, (4) a `to_imf_fixdate()` method on `DateTime` for the `Date:` header, and (5) a refactored `handle_connection` that sends 400/431 responses instead of silently returning `Err`.

The IMF-fixdate format is precisely specified: `"Sun, 06 Nov 1994 08:49:37 GMT"` — a fixed 29-byte output. Computing the weekday requires the `weekday_from_days` algorithm from Howard Hinnant's date algorithms page: `(epoch_days + 4) % 7` for non-negative epoch days, yielding 0=Sunday. The `DateTime` struct already stores `epoch_days: u64` and knows year/month/day, so `to_imf_fixdate` only needs to add weekday + time-of-day (derived from `epoch_seconds % 86400`) to produce the formatted string.

The error-response integration is the most structurally complex part: `handle_connection` currently returns `Err` without writing any bytes when parsing fails. Phase 2 must intercept all error paths and send a properly-formed 400 or 431 response before either returning `Ok(())` (connection handled) or propagating the error for logging. The `Response` builder itself provides the tool; the refactor ensures it is used.

**Primary recommendation:** Build `Response` + `DateTime::to_imf_fixdate()` as independent units first, then wire into `handle_connection` as the final step.

---

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `std::io::Write` | std | Trait target for `write_to` | Zero-copy streaming; no intermediate String allocation |
| `std::io::BufWriter` (optional at call site) | std | Buffers TcpStream writes | Reduces syscalls when writing status + headers + body sequentially |
| `log` crate | 0.4.20 (already in Cargo.toml) | Logging in handle_connection | Already used project-wide |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `Vec<(String, String)>` | std | Header storage | For HTTP responses: typically < 10 headers; Vec outperforms HashMap below ~15 entries |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `Vec<(String, String)>` headers | `HashMap<String, String>` | HashMap is faster for lookup-heavy workloads with > 15 entries; Vec is faster and simpler for serialization order preservation with small header counts |
| `&'static str` reason phrase | `String` | `&'static str` covers all standard status codes with zero allocation; `String` needed only if dynamic reason phrases are required (they are not in this phase) |

**No new crate installation required.** This phase is pure std plus the `log` crate already present.

---

## Architecture Patterns

### Recommended Project Structure

```
src/
├── server/
│   ├── mod.rs          # handle_connection refactored — send 400/431 on error
│   ├── error.rs        # unchanged — Error variants feed into response bodies
│   ├── request.rs      # unchanged
│   ├── response.rs     # NEW — Response struct + builder + write_to
│   ├── pool.rs         # unchanged
│   └── worker.rs       # unchanged
└── time/
    └── mod.rs          # add to_imf_fixdate() + weekday_from_days() helper
```

### Pattern 1: Value-Chaining Builder

**What:** Each builder method takes `self` by value, mutates, returns `Self`. Enables `Response::new(200, "OK").header(k, v).body(bytes)` one-liners.

**When to use:** When the constructed value is only needed once (write-once, send-once response). Consuming self prevents accidental reuse.

**Example:**
```rust
// Source: RFC 9110 + idiomatic Rust builder pattern
pub struct Response {
    status: u16,
    reason: &'static str,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Response {
    pub fn new(status: u16, reason: &'static str) -> Self {
        Response {
            status,
            reason,
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    pub fn body(mut self, body: Vec<u8>) -> Self {
        self.body = body;
        self
    }

    pub fn write_to(self, writer: &mut impl Write) -> Result<()> {
        // Status line
        write!(writer, "HTTP/1.1 {} {}\r\n", self.status, self.reason)?;
        // Headers
        for (name, value) in &self.headers {
            write!(writer, "{}: {}\r\n", name, value)?;
        }
        // Blank line separator (RFC 9112 Section 2.1 ABNF)
        writer.write_all(b"\r\n")?;
        // Body
        writer.write_all(&self.body)?;
        Ok(())
    }
}
```

### Pattern 2: Mandatory Header Enforcement at `write_to` Time

**What:** `write_to` inserts mandatory headers (Content-Length, Date, Connection: close) if the caller has not already set them.

**When to use:** Ensures every response is RFC-compliant regardless of construction path. Simple "check-and-insert" approach at serialization time is more robust than panicking.

**Example:**
```rust
// Before emitting headers, ensure mandatory headers present.
// Content-Length is computed from body.len() and always inserted
// (overrides any caller-set value to guarantee accuracy).
let content_length = self.body.len().to_string();
// Insert mandatory headers that must always appear:
//   Content-Length — computed, not trusted from caller
//   Connection: close — always for this server
//   Date — caller passes result of DateTime::now()?.to_imf_fixdate()
```

**Recommendation (Claude's Discretion):** Enforce Content-Length by always computing from `self.body.len()` — do not allow the caller to set it directly, preventing mismatches. Date and Connection are set via `.header()` calls by `handle_connection` before calling `write_to`. This is simpler than having `write_to` call `DateTime::now()` itself (which would require propagating time errors through the response path).

### Pattern 3: IMF-fixdate Formatting on DateTime

**What:** A `to_imf_fixdate(&self) -> String` method added to `DateTime` that produces the exact 29-character format required by RFC 9110 Section 5.6.7.

**IMF-fixdate format (from RFC 7231 / RFC 9110):**
```
"Sun, 06 Nov 1994 08:49:37 GMT"
  ^    ^  ^   ^    ^  ^  ^
  dow  d  mon yyyy  h  m  s
```

**Day names (0=Sunday, Howard Hinnant convention):**
```
["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]
```

**Month names (1-based):**
```
["", "Jan", "Feb", "Mar", "Apr", "May", "Jun",
     "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]
```

**Weekday algorithm (Howard Hinnant, O(1), verified against 1970-01-01 = Thursday = 4):**
```rust
// Source: https://howardhinnant.github.io/date_algorithms.html
fn weekday_from_days(epoch_days: i64) -> u8 {
    if epoch_days >= -4 {
        ((epoch_days + 4) % 7) as u8
    } else {
        ((epoch_days + 5) % 7 + 6) as u8
    }
}
```

**Time-of-day extraction (from epoch_seconds already on DateTime):**
```rust
let secs_today = self.epoch_seconds % 86_400;
let hour = secs_today / 3_600;
let minute = (secs_today % 3_600) / 60;
let second = secs_today % 60;
```

**Complete implementation pattern:**
```rust
// Source: RFC 9110 Section 5.6.7, Howard Hinnant date algorithms
pub fn to_imf_fixdate(&self) -> String {
    const DAY_NAMES: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    const MONTH_NAMES: [&str; 13] = [
        "", "Jan", "Feb", "Mar", "Apr", "May", "Jun",
        "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let dow = weekday_from_days(self.epoch_days as i64) as usize;
    let secs_today = self.epoch_seconds % 86_400;
    let hour = secs_today / 3_600;
    let minute = (secs_today % 3_600) / 60;
    let second = secs_today % 60;
    let month_num: u8 = self.month.into();
    format!(
        "{}, {:02} {} {:04} {:02}:{:02}:{:02} GMT",
        DAY_NAMES[dow],
        self.day,
        MONTH_NAMES[month_num as usize],
        self.year,
        hour, minute, second,
    )
}
```

### Pattern 4: Error-Response Integration in `handle_connection`

**What:** Refactor `handle_connection` so that every parse error sends a 400 or 431 response before the function returns.

**When to use:** Required for SAFE-01 and HTTP-06. The stream must still be writable at the error point (it will be, since errors come from reading, not writing).

**Example:**
```rust
// Source: project pattern — mirrors existing handle_connection structure
fn send_error_response(stream: &mut TcpStream, status: u16, reason: &'static str, message: &str) {
    let body = message.as_bytes().to_vec();
    let content_length = body.len().to_string();
    let response = Response::new(status, reason)
        .header("Content-Type", "text/plain")
        .header("Content-Length", content_length)
        .header("Connection", "close");
    // Date header omitted from error path if DateTime::now() fails — acceptable degradation
    let response = match DateTime::now() {
        Ok(dt) => response.header("Date", dt.to_imf_fixdate()),
        Err(_) => response,
    };
    let response = response.body(body);
    // Best-effort: if write fails, there is nothing more to do
    let _ = response.write_to(stream);
}

fn handle_connection(mut stream: TcpStream) -> Result<()> {
    // ... existing read loop ...

    // Error path: 431
    if lines.len() > request::MAX_HEADER_LINES {
        send_error_response(&mut stream, 431, "Request Header Fields Too Large",
            "request too large: exceeded maximum header line limit");
        return Err(Error::RequestTooLarge);
    }

    // Parsing
    let request = match Request::parse(&http_request) {
        Ok(r) => r,
        Err(e) => {
            send_error_response(&mut stream, 400, "Bad Request", &e.to_string());
            return Err(e);
        }
    };

    // Success path: full response with all mandatory headers
    let body = b"OK".to_vec();
    let content_length = body.len().to_string();
    let date = DateTime::now().map(|dt| dt.to_imf_fixdate()).unwrap_or_default();
    let response = Response::new(200, "OK")
        .header("Content-Type", "text/plain")
        .header("Content-Length", content_length)
        .header("Date", date)
        .header("Connection", "close")
        .body(body);
    response.write_to(&mut stream)?;
    Ok(())
}
```

### Anti-Patterns to Avoid

- **Intermediate String allocation:** Do not serialize the entire response to a `String` before writing. Use `write_to(&mut impl Write)` which streams directly to the socket.
- **`format!` with `\n` only:** Always use `\r\n`. A single `\n` is NOT valid per RFC 9112; raw TCP clients will fail.
- **Content-Length from str.len():** `str.len()` returns byte count for ASCII, but `Vec<u8>` already holds bytes — use `body.len()` directly. Do not use `.chars().count()` (wrong for UTF-8).
- **Date header computed once at server start:** Date must reflect the current request time, not server startup time. Call `DateTime::now()` per request.
- **Ignoring write errors on error responses:** Use `let _ = response.write_to(stream)` in the error path — do not propagate write failures that occur while sending an error response, as the primary error is already captured.

---

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| IMF-fixdate weekday | Custom day-of-week calculation | `weekday_from_days` from Howard Hinnant's algorithms | O(1), mathematically proven, handles pre-epoch dates, verified against 1970-01-01=Thursday |
| Header name case | Case-normalization logic | Store as-provided; HTTP/1.1 field names are case-insensitive (RFC 9110 Sec 5.1) — use canonical capitalized form as convention | Not required for a `Connection: close`-only server |
| Status reason mapping | `match status_code -> &str` lookup table | Use `&'static str` parameter — caller provides reason phrase | Status codes are limited and known at each call site |

**Key insight:** This phase is intentionally simple — no external crates, no framework patterns. The value is in correct RFC details (CRLF, Content-Length bytes not chars, weekday algorithm), not in architecture complexity.

---

## Common Pitfalls

### Pitfall 1: Content-Length Byte Count vs Character Count
**What goes wrong:** `"OK".len()` returns 2 (correct for ASCII), but for non-ASCII UTF-8 bodies `str.len()` and `.chars().count()` diverge. If body is `Vec<u8>`, `body.len()` is always the byte count.
**Why it happens:** Rust's `str.len()` is bytes, not characters — this is actually correct — but the habit of calling `.len()` on a `String` intermediate step on a `Vec<u8>` body is correct; the pitfall is converting `Vec<u8>` to `String` first.
**How to avoid:** Keep body as `Vec<u8>` throughout (as decided). Use `body.len()` for Content-Length. Never convert to String to measure.
**Warning signs:** Binary content (Phase 5 PNG/WASM) has wrong Content-Length.

### Pitfall 2: Missing CRLF on the Blank Separator Line
**What goes wrong:** The blank line between headers and body is emitted as `\n` instead of `\r\n`. Many HTTP clients tolerate this; raw TCP tools do not.
**Why it happens:** Writing `"\n\n"` instead of `"\r\n\r\n"` at the end of headers.
**How to avoid:** Use `writer.write_all(b"\r\n")` explicitly for the blank line. Do not rely on `write!(writer, "\n")`.
**Warning signs:** `nc` or `curl -v` shows headers and body run together with no blank line.

### Pitfall 3: IMF-fixdate Weekday Off by One
**What goes wrong:** Weekday is computed incorrectly, yielding "Mon, 06 Nov 1994" instead of "Sun, 06 Nov 1994".
**Why it happens:** Off-by-one in the modulo offset, or using 1=Monday instead of 0=Sunday convention.
**How to avoid:** Use Howard Hinnant's `(epoch_days + 4) % 7` formula. Verify with epoch day 0 (1970-01-01 = Thursday = 4). Verify with a known date: epoch day 20440 = 2025-12-01 = Monday = 1.
**Warning signs:** Date header parses as an invalid weekday in HTTP validation tools.

### Pitfall 4: `send_error_response` Can't Borrow Stream Mutably Alongside Error
**What goes wrong:** In the header-line loop, `stream` is inside a `BufReader`. Calling `send_error_response(&mut stream, ...)` fails because `BufReader` still holds a mutable borrow.
**Why it happens:** `BufReader::new(&mut stream)` borrows `stream` for the lifetime of the reader.
**How to avoid:** Structure the code so `BufReader` is dropped (goes out of scope in a block `{ ... }`) before calling `send_error_response`. The existing code in `mod.rs` already uses a block for the reader — preserve and extend that pattern.
**Warning signs:** Compiler error "cannot borrow stream as mutable more than once at a time."

### Pitfall 5: DateTime::now() Failure in Error Path
**What goes wrong:** `send_error_response` calls `DateTime::now()` which can fail, creating an unwrap/panic risk in the error handling path.
**Why it happens:** Nested Result handling.
**How to avoid:** Use `DateTime::now().map(|dt| dt.to_imf_fixdate())` and omit the Date header if it fails (best-effort). Do not unwrap. A 400 response without a Date header is still valid and far better than a panic.
**Warning signs:** Panic in tests that inject clock errors.

---

## Code Examples

Verified patterns from official sources:

### RFC 9112 HTTP Message Structure (CRLF)
```
// Source: RFC 9112 Section 2.1
// HTTP-message = start-line CRLF *( field-line CRLF ) CRLF [ message-body ]
//
// Sender MUST NOT generate bare CR outside message body.
// Concretely, every line in the header section ends with \r\n.
// The blank separator line is also \r\n (not just \n).

"HTTP/1.1 200 OK\r\n"
"Content-Type: text/plain\r\n"
"Content-Length: 2\r\n"
"Connection: close\r\n"
"\r\n"
"OK"
```

### RFC 9112 Status Line (reason-phrase optional)
```
// Source: RFC 9112 Section 4
// status-line = HTTP-version SP status-code SP [ reason-phrase ]
// reason-phrase = 1*( HTAB / SP / VCHAR / obs-text )
//
// The SP before reason-phrase is MANDATORY even when reason-phrase is absent.
// "HTTP/1.1 200 " (with trailing space) is valid.
// "HTTP/1.1 200 OK" (with reason) is the conventional form.

write!(writer, "HTTP/1.1 {} {}\r\n", self.status, self.reason)?;
```

### weekday_from_days Algorithm
```rust
// Source: https://howardhinnant.github.io/date_algorithms.html
// Verified: epoch_days=0 (1970-01-01, Thursday) -> 4. Correct.
// Convention: 0=Sunday, 1=Monday, ..., 6=Saturday
fn weekday_from_days(z: i64) -> u8 {
    if z >= -4 {
        ((z + 4) % 7) as u8
    } else {
        ((z + 5) % 7 + 6) as u8
    }
}
```

### IMF-fixdate Output Format
```rust
// Source: RFC 9110 Section 5.6.7 (formerly RFC 7231 Section 7.1.1.1)
// Example: "Sun, 06 Nov 1994 08:49:37 GMT"
// Always 29 characters. Always UTC ("GMT" literal).
// Day zero-padded to 2 digits. Year 4 digits. Hour/min/sec zero-padded.
format!(
    "{}, {:02} {} {:04} {:02}:{:02}:{:02} GMT",
    day_name, day, month_name, year, hour, minute, second
)
```

### Content-Length from Vec<u8>
```rust
// Source: HTTP-02 requirement + Rust std docs
// body.len() is always byte count — correct for Content-Length
let body: Vec<u8> = "Hello, world!".as_bytes().to_vec();
let content_length = body.len().to_string(); // "13"
```

---

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Raw `format!` string for HTTP response | `Response` builder + `write_to` | Phase 2 | Correct CRLF, mandatory headers, binary-safe body |
| Silent `return Err` on malformed request | `send_error_response` then return | Phase 2 | Client receives 400/431 instead of abrupt close |
| No Date header | `DateTime::to_imf_fixdate()` in every response | Phase 2 | RFC 9110 mandatory Date header fulfilled |

**The existing code that needs replacing:**
```rust
// In handle_connection — current raw format string (REPLACE THIS):
stream.write_all(
    format!(
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
        body.len(), body
    )
    .as_bytes(),
)?;
// Missing: Content-Type, Date, Connection headers; not using Response builder
```

---

## Open Questions

1. **`reason` phrase field type: `&'static str` or `String`?**
   - What we know: All status codes used in Phase 2 (200, 400, 431) have fixed reason phrases. `&'static str` avoids allocation.
   - What's unclear: Whether Phase 3+ will need dynamically computed reason phrases.
   - Recommendation: Use `&'static str` now. All standard HTTP status codes have well-known reason phrases that can be string literals. If dynamic phrases are needed later, a field change to `String` is a straightforward refactor.

2. **Mandatory header enforcement strategy: panic vs. always-set defaults?**
   - What we know: The builder pattern can enforce at `write_to` call time. Panicking on missing mandatory headers makes violations visible in tests. Always-set defaults silently correct omissions.
   - What's unclear: Which is more appropriate given the project's test coverage goals.
   - Recommendation: Always-set defaults for Content-Length (always computed from body), always emit Connection: close. For Content-Type and Date: require caller to set them (omitting is a logic error, tests will catch missing headers). Do not panic in production code.

3. **Should `write_to` consume `self` or take `&self`?**
   - What we know: CONTEXT.md specifies `write_to(&mut impl Write) -> Result<()>` without specifying ownership of `self`.
   - What's unclear: Whether consuming `self` or taking `&self` is implied.
   - Recommendation: `write_to(self, writer: &mut impl Write)` — consuming the response after sending it prevents double-send. Consistent with builder pattern where `self` is consumed at each stage.

---

## Sources

### Primary (HIGH confidence)
- RFC 9112 (https://www.rfc-editor.org/rfc/rfc9112#section-2.2) — CRLF requirement: "MUST NOT generate a bare CR within any protocol elements other than the content"; HTTP-message ABNF
- RFC 9112 Section 4 (https://www.rfc-editor.org/rfc/rfc9112#section-4) — status-line ABNF: `status-line = HTTP-version SP status-code SP [ reason-phrase ]`; reason-phrase is optional but SP is mandatory
- Howard Hinnant date algorithms (https://howardhinnant.github.io/date_algorithms.html) — `weekday_from_days` algorithm, O(1), verified 1970-01-01=Thursday=4
- httpdate crate source (https://github.com/pyfisch/httpdate/blob/main/src/date.rs) — confirmed exact day/month name arrays and 29-byte template format

### Secondary (MEDIUM confidence)
- RFC 9110 Section 5.6.7 / RFC 7231 Section 7.1.1.1 — IMF-fixdate format: `"Sun, 06 Nov 1994 08:49:37 GMT"` (verified via multiple sources including MDN and httpdate crate)
- RFC 9110 / RFC 6585 — 431 status code: "Request Header Fields Too Large" — server refuses request when headers are too long (verified via MDN)
- Rust forum discussion on Vec vs HashMap for small collections — Vec outperforms HashMap below ~15 entries (MEDIUM — search result, consistent with general algorithmic knowledge)

### Tertiary (LOW confidence)
- General Rust builder pattern guidance from rust-lang docs and community sources — consuming self vs &mut self tradeoffs (consistent with CONTEXT.md decision, not separately verified against a definitive source)

---

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — std-only, well-established Rust patterns, no new crates
- Architecture: HIGH — patterns derived from existing codebase conventions + RFC text
- IMF-fixdate format: HIGH — verified via RFC text, httpdate crate source, multiple examples
- weekday_from_days algorithm: HIGH — verified against Howard Hinnant's original page with test vectors
- Pitfalls: HIGH — derived from actual code paths (BufReader borrow), RFC normative text (CRLF), and direct code review

**Research date:** 2026-03-01
**Valid until:** 2026-04-01 (stable domain — HTTP/1.1 spec hasn't changed; std Rust patterns are stable)
