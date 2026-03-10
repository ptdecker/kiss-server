# Phase 2: Response and HTTP Compliance - Context

**Gathered:** 2026-03-01
**Status:** Ready for planning

<domain>
## Phase Boundary

Build a `Response` struct that serializes valid HTTP/1.1 messages with mandatory headers
(Content-Type, Content-Length, Date, Connection: close), CRLF line endings, and byte-accurate
Content-Length. Send 400 Bad Request (or 431 for oversized headers) instead of crashing or
silently closing on malformed input. Expose IMF-fixdate formatting from DateTime for the Date
header. File serving and routing belong to later phases.

</domain>

<decisions>
## Implementation Decisions

### Response struct API
- Value-chaining builder pattern: `Response::new(200, "OK").header(k, v).body(bytes)`
- Each builder method takes `self` and returns `Self` (no `&mut self`)
- Lives in `server/response.rs` — mirrors `server/request.rs`
- Body stored as `Vec<u8>` (binary-safe from day one, required for Phase 5 WASM/PNG)
- Serializes via `write_to(&mut impl Write) -> Result<()>` — streams bytes directly, no intermediate allocation

### Error response bodies
- Descriptive messages sourced from `Error::Display` (e.g., `"invalid request: bad method"`)
- Content-Type: `text/plain` for all error responses
- Error's existing Display impls flow directly into the body — no separate message mapping

### HTTP status codes
- `Error::RequestTooLarge` → **431 Request Header Fields Too Large** (precise, separate branch)
- All other parse/IO errors → **400 Bad Request**
- Malformed/non-UTF-8 input currently returns `Err` with no response; must send 400 before returning

### Claude's Discretion
- `reason` phrase field type (`&'static str` or `String`)
- IMF-fixdate method name on DateTime (e.g., `to_imf_fixdate()` or `format_http_date()`)
- How mandatory headers are enforced at `write_to` time (panic vs always-set defaults)
- Internal header storage strategy (Vec of tuples vs HashMap)

</decisions>

<specifics>
## Specific Ideas

No specific references — open to standard approaches within the decisions above.

</specifics>

<code_context>
## Existing Code Insights

### Reusable Assets
- `DateTime` in `src/time/mod.rs`: computes year/month/day/hour/min/sec from SystemTime — needs `to_imf_fixdate()` added for TIME-03
- `server::error::Error`: has `InvalidRequest`, `Channel`, `Io`, `RequestTooLarge` variants — Display impls feed directly into error response bodies
- `BufReader`-based header reading in `handle_connection`: already strips CRLF, enforces 100-line limit

### Established Patterns
- Value-chaining builders: not yet used, but idiomatic Rust; consistent with how the codebase uses trait bounds and generic parameters
- Module structure: `server/request.rs` for `Request` → `server/response.rs` for `Response` mirrors exactly
- Error type aliases: `pub type Result<T> = std::result::Result<T, Error>` — Response will use the same pattern
- `write_all` on TcpStream: already used in `handle_connection` for the current raw format string response

### Integration Points
- `handle_connection` in `server/mod.rs`: current response construction (raw format string) gets replaced with `Response` builder + `write_to`
- Error path in `handle_connection`: currently returns `Err` silently; needs to send 400/431 before returning
- `DateTime::now()` call site: needs `to_imf_fixdate()` result passed as the `Date` header value
- Phase 3 `Context` struct: will hold a `Response` — the builder API chosen here defines that contract

</code_context>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 02-response-and-http-compliance*
*Context gathered: 2026-03-01*
