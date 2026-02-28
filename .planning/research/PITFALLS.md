# Domain Pitfalls: Pure-Rust HTTP/1.1 Static File Server

**Domain:** Pure-Rust HTTP/1.1 static file server (no external dependencies)
**Researched:** 2026-02-28
**Confidence:** HIGH — all findings are drawn from Rust stdlib documentation,
RFC 3986/9110/9112 specifications, and verified against the existing codebase
analysis in CONCERNS.md. No web searches were used; confidence is based on
authoritative specification knowledge and Rust stdlib behavior.

---

## Critical Pitfalls

Mistakes that cause security incidents, correctness failures, or require rewrites.

---

### Pitfall 1: Path Traversal via `..` Segments After Percent-Decode

**What goes wrong:**
A client sends `GET /%2e%2e/%2e%2e/etc/passwd HTTP/1.1`. The server percent-decodes
the path first (producing `../../etc/passwd`), then passes the decoded string to
`root.join(decoded_path)`. The `PathBuf::join()` call happily traverses above the
root. The server serves `/etc/passwd` with a 200 OK.

A subtler variant: the client sends `GET /static/../../../../etc/passwd`. After join,
the path escapes the root but no `..` normalization is applied before the file is read.

Another variant exploits how `Path::join()` handles absolute paths in Rust: if the
decoded path starts with `/`, `PathBuf::join("/etc/passwd")` discards the root entirely
and resolves to `/etc/passwd`. This is Rust-specific and surprises many implementors.

**Why it happens:**
Developers normalize paths lexically (stripping `..` with string manipulation), or
check the path string before decoding rather than after. Neither is sufficient.
The `PathBuf::join()` absolute-path behavior is not obvious from the name.

**Consequences:**
Arbitrary file read anywhere on the server filesystem the process has read access to.
On a typical development machine this means all user files. Critical security failure.

**Prevention (pure-Rust, no deps):**
1. Percent-decode the path first. Never pass a percent-encoded string to filesystem APIs.
2. Strip any leading `/` from the decoded path before calling `root.join()`. This prevents
   `PathBuf::join()` from discarding the root on absolute paths.
3. After `root.join(stripped_path)`, call `fs::canonicalize()` on the result. This
   resolves all `..` components and symlinks, producing the true absolute path.
4. Verify the canonicalized path starts with the canonicalized root:
   ```rust
   let root_canon = fs::canonicalize(&self.root)?;
   let file_canon = fs::canonicalize(&full_path)?;
   if !file_canon.starts_with(&root_canon) {
       return Ok(Response::forbidden());
   }
   ```
5. Canonicalize the root in `StaticFileHandler::new()`, not on every request, to avoid
   redundant syscalls.

**Warning signs:**
- Any code that calls `root.join(user_input)` without a subsequent `starts_with` check.
- String manipulation to remove `..` (e.g., `replace("../", "")`) before the join.
- Checking for `..` before percent-decoding.

**Phase:** Phase 5 (StaticFileHandler) — must be in the initial implementation, not
added later. This is not an optimization; it is a correctness requirement.

---

### Pitfall 2: `fs::canonicalize()` Fails on Non-Existent Paths

**What goes wrong:**
The path traversal check described in Pitfall 1 relies on `fs::canonicalize()`.
However, `canonicalize()` calls `realpath(3)` on Unix, which returns an error if any
component of the path does not exist on disk. If the file does not exist,
`canonicalize()` returns `Err(NotFound)`, and a naive implementation either panics,
returns a 500, or skips the security check entirely.

If the error is caught and the handler returns 404, that is actually safe — but the
code path that skips the security check to "just return 404 faster" is a vulnerability.

**Why it happens:**
Developers test with files that exist. The `canonicalize()` failure path only triggers
for missing files, which is the exact case that looks like a benign 404.

**Consequences:**
If the traversal check is skipped on `canonicalize` failure, an attacker can probe for
non-existent files outside the root without triggering the check. More critically, on
some platforms the traversal check can be bypassed entirely if the attacker crafts a
path where `canonicalize` fails for a reason other than "not found."

**Prevention (pure-Rust, no deps):**
Check traversal on the parent directory (which must exist for a valid file), or use a
two-phase approach:
1. Validate path components before canonicalization: reject any decoded path segment
   that equals `..` or `.` after splitting on `/`. This is a defense-in-depth check.
2. Attempt `canonicalize()`. If it returns `NotFound`, return 404 — but only after
   the component-level check has already passed.
3. Alternatively: canonicalize the parent directory chain until you reach the root,
   and verify that chain stays within the root.

The simplest safe implementation:
```rust
// Step 1: reject ".." components explicitly
let decoded = request.target.decoded_path()?;
let decoded = decoded.trim_start_matches('/');
if decoded.split('/').any(|seg| seg == ".." || seg == ".") {
    return Ok(Response::not_found());  // 404, not 403 — don't reveal structure
}

// Step 2: build full path
let full_path = self.root.join(decoded);

// Step 3: try to canonicalize; file-not-found is a normal 404
let file_canon = match fs::canonicalize(&full_path) {
    Ok(p) => p,
    Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Response::not_found()),
    Err(e) => return Err(e.into()),
};

// Step 4: verify containment (belt + suspenders)
if !file_canon.starts_with(&self.root_canon) {
    return Ok(Response::not_found());
}
```

**Warning signs:**
- `canonicalize()` result used directly without checking the error case.
- A `?` operator after `canonicalize()` that would return 500 for missing files.
- Any early-return that skips the `starts_with` check.

**Phase:** Phase 5 (StaticFileHandler) — same phase as Pitfall 1.

---

### Pitfall 3: Panic in Worker Threads Poisons the Mutex and Kills All Workers

**What goes wrong:**
The existing worker threads use `.expect("unable to lock spawned thread")` when
acquiring the job queue mutex. If any worker panics while holding the lock, the mutex
becomes poisoned. The next `.lock()` call returns `Err(PoisonError)`. The `.expect()`
call on that `Err` panics the second worker. This cascades: all workers panic in
sequence, leaving the thread pool dead. The `TcpListener` keeps accepting connections
but no workers are alive to handle them — the server silently hangs.

This is already identified in CONCERNS.md but is critical enough to document in full
for roadmap planning.

**Why it happens:**
`Mutex::lock()` returns a `Result` specifically to communicate poisoning. Calling
`.expect()` or `.unwrap()` converts this into a panic. Combined with a panic in a
worker, this turns one failure into a total server crash.

**Consequences:**
One malformed request that causes a worker to panic kills the entire server silently.
No log output, no error to the client, no recovery.

**Prevention (pure-Rust, no deps):**
Handle the poison case explicitly in the worker loop:
```rust
loop {
    let job = match receiver.lock() {
        Ok(guard) => guard.recv(),
        Err(poisoned) => {
            // Log the poison; recover the inner data and continue
            log::warn!("Worker recovered from poisoned mutex");
            poisoned.into_inner().recv()
        }
    };

    match job {
        Ok(job) => job(),
        Err(_) => break,  // Channel closed: shutdown
    }
}
```

Additionally, in `ThreadPool::drop()`, never call `.expect()` or `.unwrap()` on
`thread.join()`. Use `let _ = thread.join()` — a panicking thread has already done
its damage; re-panicking in `Drop` causes a double-panic, which aborts the process.

**Warning signs:**
- `.expect()` or `.unwrap()` on any `Mutex::lock()` result in worker code.
- `.expect()` in a `Drop` implementation.
- No test that sends a malformed request and verifies the server still responds.

**Phase:** Phase 1 (bug fixes) — must be fixed before any new features are added.

---

### Pitfall 4: Unbounded Request Reading Enables Memory Exhaustion

**What goes wrong:**
The existing request reader calls `BufReader::lines()` and collects lines until a blank
line. An attacker sends an HTTP request with an infinite stream of headers — no blank
line ever arrives. The server thread reads forever, consuming memory until the process
OOMs or the connection is dropped by the OS.

With a 4-thread pool, four concurrent infinite requests exhaust all workers.

**Why it happens:**
`BufReader::lines()` is a lazy iterator; the loop only terminates when a blank line is
seen. If no blank line is sent, the loop never terminates.

**Consequences:**
Complete denial of service with 4 connections. Even with a larger thread pool, a
small number of connections can exhaust all workers.

**Prevention (pure-Rust, no deps):**
Enforce a maximum header count and a maximum header line size during parsing:
```rust
const MAX_HEADERS: usize = 100;
const MAX_HEADER_LINE_BYTES: usize = 8192;  // 8 KB per header line

let mut lines = Vec::new();
for line in reader.lines().take(MAX_HEADERS + 1) {
    let line = line?;
    if line.len() > MAX_HEADER_LINE_BYTES {
        return Err(Error::RequestTooLarge);
    }
    if line.is_empty() {
        break;
    }
    lines.push(line);
}
```

The `take(MAX_HEADERS + 1)` ensures the loop terminates even with no blank line.
Return `400 Bad Request` when the limit is hit.

**Warning signs:**
- Any `for line in reader.lines()` loop without a count limit or `take()`.
- No test for a request with no terminating blank line.
- No test for a request with thousands of headers.

**Phase:** Phase 1 (bug fixes) — this is a DoS vector present in the existing code.

---

### Pitfall 5: Non-UTF-8 Request Data Panics the Parser

**What goes wrong:**
`BufReader::lines()` returns `io::Result<String>`. HTTP headers are ASCII, but clients
(or attackers) can send binary data. A non-UTF-8 byte sequence causes `lines()` to
return `Err(InvalidData)`. The existing code calls `.unwrap()` on this result, causing
a worker thread panic.

**Why it happens:**
`BufReader::lines()` uses `String::from_utf8_lossy()` internally for UTF-8 detection
but returns an error for invalid sequences. `.unwrap()` is a typical first-draft choice.

**Consequences:**
Any client (or attacker) sending a single binary byte in the request line crashes a
worker thread, which then poisons the mutex (see Pitfall 3).

**Prevention (pure-Rust, no deps):**
Propagate the error with `?` rather than panicking:
```rust
let line = line.map_err(|e| Error::Io(e))?;
```

Or use `read_until(b'\n')` on the `BufReader` and convert bytes manually, rejecting
non-ASCII with a 400 response:
```rust
if !line.is_ascii() {
    return Err(Error::BadRequest("non-ASCII in headers"));
}
```

Either approach converts a panic into a recoverable 400 error.

**Warning signs:**
- Any `.unwrap()` or `.expect()` on `io::Result<String>` from `BufReader::lines()`.
- No test sending binary data to the request parser.

**Phase:** Phase 1 (bug fixes) — existing known bug in CONCERNS.md.

---

## Moderate Pitfalls

Mistakes that produce incorrect behavior or spec violations, but do not crash the server.

---

### Pitfall 6: Missing or Wrong `Content-Length` Breaks HTTP/1.1 Compliance

**What goes wrong:**
HTTP/1.1 requires `Content-Length` for responses with a body when transfer-encoding is
not chunked (RFC 9112 §6.3). Without it, the client does not know where the response
body ends. Browsers typically buffer until the connection closes, which works for
HTTP/1.0-style close-after-response but introduces latency and prevents any future
keep-alive behavior. Some clients (curl, test harnesses) will report a protocol error.

The `Content-Length` must be the byte length of the body (`Vec<u8>::len()`), not the
character count of a string. For a UTF-8 HTML file, these differ when the file contains
multi-byte characters.

**Why it happens:**
Developers use `String::len()` instead of `as_bytes().len()` or `Vec<u8>::len()`.
Or they forget to set `Content-Length` at all in convenience constructors.

**Prevention (pure-Rust, no deps):**
- The `Response::set_body()` method should automatically set `Content-Length`:
  ```rust
  pub fn set_body(&mut self, body: Vec<u8>) {
      self.set_header("Content-Length", body.len().to_string());
      self.body = body;
  }
  ```
- Never compute `Content-Length` from a `String`. Always use the serialized byte length.
- In `to_bytes()`, assert that `Content-Length` is present when body is non-empty, or
  auto-compute it as a safety net (with a debug assertion).

**Warning signs:**
- `Content-Length` set from `string.len()` rather than `bytes.len()`.
- Convenience constructors that set a string body without setting `Content-Length`.
- curl reporting `curl: (18) transfer closed with outstanding read data remaining`.

**Phase:** Phase 2 (Response struct) — build it in correctly from the start.

---

### Pitfall 7: `Content-Type` Missing `charset=utf-8` for Text Responses

**What goes wrong:**
Without `; charset=utf-8` in the `Content-Type` header for text types, browsers may
mis-detect the encoding of HTML files. RFC 9110 §8.3 allows content negotiation, but
for a static file server serving known-UTF-8 files, always declaring the charset
prevents browser rendering bugs. More critically, `text/html` without charset causes
some security scanners to flag XSS risks (charset sniffing attacks).

**Why it happens:**
MIME type tables list `text/html` without the charset parameter. It is easy to copy
incomplete MIME type strings.

**Prevention (pure-Rust, no deps):**
Include `; charset=utf-8` in the MIME type string for all `text/*` types:
```rust
"html" | "htm" => "text/html; charset=utf-8",
"css"          => "text/css; charset=utf-8",
"js"           => "application/javascript",  // charset not required for JS
"txt"          => "text/plain; charset=utf-8",
```

**Warning signs:**
- MIME map with `"text/html"` (no charset).
- Browser showing a question mark or mojibake for a UTF-8 HTML file with non-ASCII content.

**Phase:** Phase 5 (StaticFileHandler/MIME detection).

---

### Pitfall 8: `Date` Header Missing from Responses

**What goes wrong:**
RFC 9110 §6.6.1 requires an origin server to send a `Date` header in all responses
except 1xx, 204, and 304. Browsers and CDNs use `Date` for caching, request/response
correlation, and RFC compliance checks. Without it, caching proxies may misbehave, and
some strict HTTP test suites will fail.

The project already has a `DateTime` struct — this is a wiring problem, not a new
implementation problem.

**Why it happens:**
The `Date` header is added to a list of "do later" items. The server works without it
because browsers tolerate its absence.

**Prevention (pure-Rust, no deps):**
Add `Date` in `Response::new()` or in a `Response::standard_headers()` helper that
all response constructors call:
```rust
pub fn new(status: u16, reason: &'static str) -> Self {
    let mut r = Response { status, reason, headers: Vec::new(), body: Vec::new() };
    r.set_header("Date", DateTime::now().to_http_date());
    r.set_header("Server", "ptodd/0.1");
    r
}
```

This ensures `Date` is never forgotten on any response path.

**Warning signs:**
- `curl -I` output with no `Date:` header.
- `Response::new()` that sets no default headers.

**Phase:** Phase 2 (Response struct).

---

### Pitfall 9: Serving Directories as Files Returns Garbage or Panics

**What goes wrong:**
A client requests `GET /images/ HTTP/1.1` (a directory, not a file). `fs::read()` on a
directory returns `Ok` on some platforms with an empty or garbage result, and
`Err(IsADirectory)` on others (Linux returns `EISDIR`). If the error is not handled, the
server either panics or returns a 200 with an empty body. A 404 or 405 is correct.

**Why it happens:**
`StaticFileHandler` is only tested with file paths. Directory paths are an edge case.

**Consequences:**
Undefined response for directory requests; potential panic on platforms where `fs::read()`
does not return `IsADirectory`.

**Prevention (pure-Rust, no deps):**
Check `fs::metadata()` before reading:
```rust
let meta = match fs::metadata(&file_canon) {
    Ok(m) => m,
    Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Response::not_found()),
    Err(e) => return Err(e.into()),
};

if meta.is_dir() {
    return Ok(Response::not_found());  // 404; no directory listing
}

let body = fs::read(&file_canon)?;
```

This also gives a natural extension point if directory index files (`index.html`) are
added later: check for `file_canon.join("index.html")` before returning 404.

**Warning signs:**
- `StaticFileHandler` that calls `fs::read()` without checking `is_dir()` first.
- No test with a request for a directory path.

**Phase:** Phase 5 (StaticFileHandler).

---

### Pitfall 10: `PathBuf::join()` Discards the Root on Absolute Input

**What goes wrong:**
This is a Rust-specific behavior documented in `std::path::PathBuf::push()`:
"If pushed path is absolute, it replaces the entire PathBuf." Since `join()` uses
`push()` internally, `PathBuf::from("/var/www").join("/etc/passwd")` produces
`PathBuf::from("/etc/passwd")`, not `/var/www/etc/passwd`.

A client sends `GET /etc/passwd HTTP/1.1`. After percent-decoding, the path is
`/etc/passwd`. Passed directly to `root.join("/etc/passwd")`, the root is discarded.
Even if `canonicalize()` and `starts_with()` checks are in place, they run against
`/etc/passwd` directly.

**Why it happens:**
The behavior is documented but counterintuitive. Developers from other languages expect
`join` to append, not replace on absolute paths.

**Consequences:**
The `starts_with(root)` check will fail and return 403/404, which is safe, but only
because the check exists. If the check is ever bypassed or reordered, this becomes a
direct path traversal.

**Prevention (pure-Rust, no deps):**
Always strip the leading `/` from the URL-decoded path before calling `join()`:
```rust
let decoded = request.target.decoded_path()?;
let relative = decoded.trim_start_matches('/');
let full_path = self.root.join(relative);
```

This converts an absolute decoded path into a relative one that `join()` will append
to the root correctly. Document this step with a comment explaining the `join()` behavior.

**Warning signs:**
- `root.join(decoded_path)` where `decoded_path` still has a leading `/`.
- A test with `GET /etc/passwd` that returns 200 instead of 403/404.

**Phase:** Phase 5 (StaticFileHandler) — defense in depth alongside Pitfall 1.

---

### Pitfall 11: Response `to_bytes()` Uses `\n` Instead of `\r\n`

**What goes wrong:**
HTTP/1.1 requires `\r\n` (CRLF) as the line terminator for the status line and all
headers (RFC 9112 §2.2). Using `\n` (LF only) works with lenient clients like browsers
but fails with strict clients, load balancers, and RFC-compliance test suites.

The `\r\n` after the last header and the blank `\r\n` that separates headers from the
body are especially easy to get wrong — one missing `\r\n` causes the client to treat
the body as part of the headers.

**Why it happens:**
Rust string literals use `\n` by default. It is easy to write `format!("{name}: {value}\n")`
instead of `format!("{name}: {value}\r\n")`.

**Prevention (pure-Rust, no deps):**
Define a constant and test the raw bytes:
```rust
const CRLF: &[u8] = b"\r\n";

fn to_bytes(&self) -> Vec<u8> {
    let mut out = Vec::new();
    // Status line
    out.extend_from_slice(
        format!("HTTP/1.1 {} {}", self.status, self.reason).as_bytes()
    );
    out.extend_from_slice(CRLF);
    // Headers
    for (name, value) in &self.headers {
        out.extend_from_slice(format!("{}: {}", name, value).as_bytes());
        out.extend_from_slice(CRLF);
    }
    // Blank line
    out.extend_from_slice(CRLF);
    // Body
    out.extend_from_slice(&self.body);
    out
}
```

Write a unit test that inspects `to_bytes()` output at the byte level to verify `\r\n`.

**Warning signs:**
- `format!` with `\n` in response serialization code.
- curl showing truncated or merged headers and body.

**Phase:** Phase 2 (Response struct).

---

### Pitfall 12: Percent-Decode Before Routing, Not After

**What goes wrong:**
If the router compares percent-encoded paths for matching (`/images%2Flogo.png` vs
`/images/logo.png`), routes will fail to match for encoded requests. Conversely, if the
router decodes before matching and a route pattern contains an encoded character, the
match will be inconsistent.

Worse: if routing is done on the decoded path, then the decoded path is passed to the
filesystem handler without re-checking for `..` or absolute path components that the
decode may have introduced.

**Why it happens:**
The decode step is applied inconsistently — sometimes before routing, sometimes before
filesystem resolution, sometimes both.

**Prevention (pure-Rust, no deps):**
Define a clear two-stage pipeline:
1. **Router matching uses the raw (encoded) path.** Route patterns are written with the
   same encoding as expected incoming requests (typically no encoding in patterns).
2. **File handlers decode the path** using `Url::decoded_path()` immediately before
   filesystem resolution, and validate the decoded result before any `join()` call.

Never pass a decoded path to the router for matching. Never pass an encoded path to
the filesystem. Make this explicit in the `Handler` trait documentation.

**Warning signs:**
- Calling `decoded_path()` in `Router::dispatch()` before calling the handler.
- A handler that receives an already-decoded path via the `Request` struct.
- Router patterns that contain `%2F` or other encoded characters.

**Phase:** Phase 4 (URL methods) and Phase 5 (StaticFileHandler) — the decode contract
must be decided when `Url::decoded_path()` is implemented.

---

## Minor Pitfalls

Issues that are annoying but do not cause security or correctness failures.

---

### Pitfall 13: `unsafe { unwrap_unchecked() }` in DateTime

**What goes wrong:**
`std::time::SystemTime::duration_since(UNIX_EPOCH)` returns `Result<Duration, SystemTimeError>`.
The `SystemTimeError` occurs when the system clock is set to before the Unix epoch (Jan 1, 1970).
Using `unsafe { unwrap_unchecked() }` instead of proper error handling is undefined behavior
if the condition is ever false — even on systems where it would normally be safe.

**Prevention (pure-Rust, no deps):**
Replace with a safe fallback:
```rust
let duration = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap_or(Duration::ZERO);  // Fallback: epoch time if clock is broken
```

Or propagate the error if `DateTime::now()` returns `Result<DateTime>`.

**Warning signs:**
- Any `unsafe { ... unwrap_unchecked() ... }` in `src/time/mod.rs`.

**Phase:** Phase 1 (bug fixes).

---

### Pitfall 14: Double-Panic in `ThreadPool::drop()`

**What goes wrong:**
`Drop` implementations must never panic. If `thread.join()` returns `Err` (because the
thread panicked), calling `.expect()` in `Drop` causes a second panic. If the `Drop` is
triggered during unwinding from the first panic, Rust aborts the process rather than
propagating either panic.

**Prevention (pure-Rust, no deps):**
```rust
impl Drop for ThreadPool {
    fn drop(&mut self) {
        // ... send shutdown signal ...
        for worker in &mut self.workers {
            if let Some(thread) = worker.thread.take() {
                let _ = thread.join();  // Ignore join errors; thread already panicked
            }
        }
    }
}
```

**Warning signs:**
- `.expect()` or `.unwrap()` on `thread.join()` in any `Drop` implementation.

**Phase:** Phase 1 (bug fixes).

---

### Pitfall 15: `MIME` Type `application/javascript` vs `text/javascript`

**What goes wrong:**
The official IANA-registered MIME type for JavaScript is `text/javascript` (as of
RFC 4329, superseded by RFC 9239 in 2022). `application/javascript` is deprecated but
still widely accepted. Using `application/javascript` will produce warnings in some
strict browsers (Firefox devtools) and may cause Content Security Policy issues.

**Prevention (pure-Rust, no deps):**
Use `text/javascript` in the MIME map:
```rust
"js" | "mjs" => "text/javascript",
```

**Warning signs:**
- `"application/javascript"` in the MIME type map.
- Firefox console warnings about MIME type mismatch.

**Phase:** Phase 5 (MIME detection).

---

### Pitfall 16: Year/Month Calculation Loops Produce Wrong `Date` Header Near Year Boundaries

**What goes wrong:**
The existing `DateTime::now()` iterates from 1970 to compute the current year. This loop
is correct for most dates but is sensitive to off-by-one errors in leap year handling
near December 31 / January 1 boundaries. If the `Date` header value is wrong (e.g., one
day off), HTTP caches may incorrectly cache or reject responses. The existing CONCERNS.md
identifies this as a performance issue; it is also a correctness risk.

**Prevention (pure-Rust, no deps):**
Replace the loops with the standard astronomical formula:
- Years since epoch: use `days / 365.2425` as an initial estimate, then correct.
- Or use the proleptic Gregorian algorithm (Zeller's formula / Julian day number).

Add tests for: Dec 31, Jan 1, Feb 28, Feb 29 on leap years, and Dec 31 on year preceding a leap year.

**Warning signs:**
- A loop in `src/time/mod.rs` that iterates from 1970 to the current year.
- Absence of tests for dates near year and month boundaries.

**Phase:** Phase 1 (bug fixes / DateTime arithmetic).

---

## Phase-Specific Warnings

| Phase Topic | Likely Pitfall | Mitigation |
|-------------|---------------|------------|
| Response struct | Missing or wrong `\r\n` terminators (Pitfall 11) | Unit-test `to_bytes()` at byte level |
| Response struct | `Content-Length` computed from string not bytes (Pitfall 6) | Use `Vec<u8>::len()` always; set in `set_body()` |
| Response struct | `Date` header missing (Pitfall 8) | Add in `Response::new()` |
| Bug fixes | Worker panic cascades (Pitfall 3) | Handle `PoisonError` in worker loop |
| Bug fixes | Non-UTF-8 panics (Pitfall 5) | Propagate `?` on `lines()` |
| Bug fixes | Unbounded header read (Pitfall 4) | Add `take(MAX_HEADERS)` |
| Bug fixes | `unsafe unwrap_unchecked` (Pitfall 13) | Replace with `unwrap_or` |
| Bug fixes | `Drop` double-panic (Pitfall 14) | Use `let _ = thread.join()` |
| URL methods | Decode contract (Pitfall 12) | Decode in handler, not router |
| StaticFileHandler | Path traversal via `..` (Pitfall 1) | `canonicalize()` + `starts_with()` |
| StaticFileHandler | `canonicalize()` on missing file (Pitfall 2) | Check `NotFound` error; component-level `..` rejection |
| StaticFileHandler | Absolute URL path discards root (Pitfall 10) | `trim_start_matches('/')` before `join()` |
| StaticFileHandler | Serving directories (Pitfall 9) | `fs::metadata().is_dir()` check |
| StaticFileHandler | MIME charset missing (Pitfall 7) | Include `; charset=utf-8` for text/* |
| MIME detection | Deprecated `application/javascript` (Pitfall 15) | Use `text/javascript` |
| DateTime | Boundary bugs in `Date` header (Pitfall 16) | Replace loops with calendar arithmetic |

---

## Security Pitfalls Summary

The following pitfalls are security-relevant and must be treated as blockers, not
polish:

| # | Pitfall | Severity | Phase to Fix |
|---|---------|----------|--------------|
| 1 | Path traversal via `..` after percent-decode | Critical | Phase 5 |
| 2 | `canonicalize()` failure bypasses traversal check | Critical | Phase 5 |
| 4 | Unbounded header read (DoS) | High | Phase 1 |
| 5 | Non-UTF-8 panics worker (DoS via cascade) | High | Phase 1 |
| 10 | `PathBuf::join()` discards root on absolute path | High | Phase 5 |
| 3 | Mutex poison cascades (DoS amplifier) | Medium | Phase 1 |

Pitfalls 1, 2, and 10 must all be present simultaneously in `StaticFileHandler` for
the path traversal defense to be robust. No single check is sufficient alone.

---

## Sources

All findings are based on:

- RFC 9110 (HTTP Semantics), RFC 9112 (HTTP/1.1 Message Syntax), RFC 3986 (URI Generic Syntax) — specification requirements
- RFC 9239 (Updates to ECMAScript Media Types) — `text/javascript` IANA registration
- Rust `std::path::PathBuf` documentation — `join()` absolute-path replacement behavior
- Rust `std::fs::canonicalize()` documentation — `realpath(3)` requirement for existing path
- Rust `std::sync::Mutex` documentation — `PoisonError` behavior and recovery
- CONCERNS.md — existing codebase analysis (2026-02-28)
- ARCHITECTURE.md — planned component structure (2026-02-28)

Confidence: HIGH for Pitfalls 1–14 (all grounded in Rust stdlib behavior and HTTP RFCs).
Confidence: MEDIUM for Pitfall 15 (MIME type registry change, widely adopted but
implementation details vary by client).

---

*Research completed: 2026-02-28*
