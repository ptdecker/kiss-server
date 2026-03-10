# Technology Stack

**Project:** ptodd — Pure-Rust HTTP/1.1 Static File Server
**Research focus:** std-only APIs for static file serving milestone
**Researched:** 2026-02-28
**Overall confidence:** HIGH — Rust std is stable; all claims are from stable APIs present since Rust 1.56 (2021 edition minimum).

---

## Existing Stack (Locked — Do Not Change)

| Technology | Version | Role |
|------------|---------|------|
| Rust | 2021 edition | Language |
| `log` v0.4.20 | features = ["std"] | Logging facade |
| Custom `ThreadPool` | in-tree | Connection concurrency |
| Custom `DateTime` | in-tree | RFC-9110 `Date` header |
| Custom `Url` | in-tree | Request target storage + pct-encode/decode |

No new dependencies may be added. All APIs below are from `std`.

---

## Recommended std APIs by Concern

### 1. File I/O

**Primary module:** `std::fs`

The correct approach for serving files is streaming via `std::fs::File` rather than loading to `String`. `fs::read_to_string` (currently used in `handle_connection`) fails on binary files (WASM, images, fonts) and forces the full file into memory before writing.

| API | Use | Notes |
|-----|-----|-------|
| `std::fs::File::open(path)` | Open a file for reading | Returns `io::Result<File>` — map `Err` to 404 |
| `std::fs::File::metadata()` | Get file size for `Content-Length` | `metadata().len()` returns `u64` |
| `std::fs::metadata(path)` | Stat a path (without opening) | Use to distinguish file vs dir, check existence |
| `std::io::Read` trait | Read bytes from File | Implement via `file.read(&mut buf)` in a loop |
| `std::io::copy(&mut src, &mut dst)` | Stream file to socket | Most efficient: no full-file allocation |
| `std::io::BufWriter` | Buffer socket writes | Wrap `TcpStream` to batch header + body writes |

**Correct serving pattern:**
```rust
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::net::TcpStream;

fn serve_file(stream: TcpStream, path: &std::path::Path) -> io::Result<()> {
    let mut file = File::open(path)?;
    let size = file.metadata()?.len();
    let mime = mime_for_path(path);

    let mut writer = BufWriter::new(stream);
    write!(writer,
        "HTTP/1.1 200 OK\r\nContent-Type: {mime}\r\nContent-Length: {size}\r\n\r\n"
    )?;
    io::copy(&mut file, &mut writer)?;
    writer.flush()
}
```

**What you CANNOT do with std only:**
- Range requests (`Range:` / `Content-Range:`) — need manual byte-range seeking via `std::io::Seek`; doable but complex
- Async sendfile — no `sendfile(2)` syscall wrapper; `io::copy` is the portable equivalent

---

### 2. Path Handling and Path Traversal Prevention

**Primary modules:** `std::path::{Path, PathBuf}`

Path traversal (e.g. `GET /../etc/passwd`) is the single most critical security concern for a static file server. The correct prevention strategy is `Path::canonicalize` + prefix check.

| API | Use | Notes |
|-----|-----|-------|
| `std::path::Path` | Borrowed path type | Zero-copy path operations |
| `std::path::PathBuf` | Owned path buffer | Heap-allocated path for building |
| `PathBuf::push(component)` | Append a path segment | Use to join root + request path |
| `Path::canonicalize()` | Resolve all `..`, symlinks, relative refs | Returns `io::Result<PathBuf>` — fails if path does not exist |
| `Path::starts_with(prefix)` | Check that resolved path is under root | Core traversal check |
| `Path::extension()` | Get file extension as `OsStr` | Use for MIME detection |
| `Path::is_file()` | True if path is a regular file | Distinguish from directories |
| `Path::components()` | Iterate path segments | Alternative traversal check pre-canonicalize |

**Correct traversal prevention:**
```rust
use std::path::{Path, PathBuf};

fn safe_path(root: &Path, request_path: &str) -> Option<PathBuf> {
    // Strip leading '/' so join doesn't replace root
    let rel = request_path.trim_start_matches('/');

    // Construct candidate: root + request path
    let candidate = root.join(rel);

    // canonicalize resolves all ".." and symlinks; returns Err if not found
    let canonical = candidate.canonicalize().ok()?;

    // Verify resolved path is under the root (also canonicalized at startup)
    if canonical.starts_with(root) {
        Some(canonical)
    } else {
        None  // Traversal attempt — return None → 403
    }
}
```

**Critical caveat:** `canonicalize` requires the path to exist on disk. For a 404 response to a non-existent file, the `Err` from `canonicalize` is the correct signal. Never serve a 404 by canonicalizing a non-existent path and trusting the `PathBuf` — the `?` / `.ok()?` handles this correctly.

**What you CANNOT do with std only:**
- Symlink-aware root confinement on Windows with junctions — `canonicalize` behaves correctly on macOS/Linux; Windows behavior is documented but subtly different for junctions.

---

### 3. MIME Type Detection

**No std API exists for MIME detection.** This must be implemented manually. This is the area where the no-external-deps constraint has the most cost.

**Approach:** A `match` on `Path::extension()` returning `&'static str`.

```rust
use std::path::Path;

fn mime_for_path(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") | Some("htm") => "text/html; charset=utf-8",
        Some("css")                => "text/css; charset=utf-8",
        Some("js")  | Some("mjs") => "text/javascript; charset=utf-8",
        Some("json")               => "application/json",
        Some("wasm")               => "application/wasm",
        Some("png")                => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif")                => "image/gif",
        Some("svg")                => "image/svg+xml",
        Some("ico")                => "image/x-icon",
        Some("txt")                => "text/plain; charset=utf-8",
        Some("xml")                => "application/xml",
        Some("pdf")                => "application/pdf",
        Some("woff")               => "font/woff",
        Some("woff2")              => "font/woff2",
        Some("ttf")                => "font/ttf",
        Some("otf")                => "font/otf",
        Some("mp4")                => "video/mp4",
        Some("webm")               => "video/webm",
        Some("mp3")                => "audio/mpeg",
        Some("ogg")                => "audio/ogg",
        _                          => "application/octet-stream",
    }
}
```

**What you CANNOT do with std only:**
- Content sniffing (reading file magic bytes to detect type) — technically doable with `std::io::Read` but not worth the complexity; extension-based detection is the correct default for a static file server.
- Comprehensive IANA MIME registry — the match table above covers the set meaningful for a web-serving use case; anything exotic maps to `application/octet-stream` which is correct browser behavior (forces download).

---

### 4. Signal Handling (Graceful Shutdown)

**Module:** `std::sync::atomic` + platform-specific constraints

The Rust standard library does **not** provide signal handling. `std::process::exit` terminates without cleanup. POSIX signal handling from pure `std` requires using `signal_hook`, `ctrlc`, or unsafe `libc`.

**Available without unsafe:**
- `std::sync::atomic::AtomicBool` — a flag polled in the accept loop
- Ctrl+C (SIGINT) in a terminal can be detected by checking `TcpListener::incoming()` loop termination

**Practical pure-std approach (no graceful SIGTERM):**
```rust
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

static RUNNING: AtomicBool = AtomicBool::new(true);

// In the accept loop:
for stream_result in listener.incoming() {
    if !RUNNING.load(Ordering::Relaxed) {
        break;
    }
    // ... handle stream
}
```

The problem: `listener.incoming()` blocks on `accept(2)`. The `AtomicBool` is only checked between accepts. SIGTERM still kills the process uncleanly mid-request unless you set a read timeout or use unsafe signal handling.

**Most practical pure-std solution:**
```rust
// Set a timeout so the accept loop has a chance to check the flag
listener.set_ttl(10)?; // Not useful here

// Use accept() in a loop with SO_RCVTIMEO via set_read_timeout
use std::time::Duration;
listener.set_nonblocking(false)?;
// accept() will block indefinitely; graceful shutdown requires unsafe
// OR: accept SIGINT/SIGTERM kills the process, which is acceptable for a v1 server
```

**Honest assessment:** True graceful SIGTERM handling (POSIX `signal(2)` / `sigaction(2)`) requires either:
1. Unsafe `extern "C"` signal handler with a global `AtomicBool` (doable in std, requires `unsafe`)
2. A `libc` or `signal-hook` crate (adds a dependency)

For this project, the pragmatic decision is: **accept that SIGINT/SIGTERM terminates the process**. The `Drop` impl on `ThreadPool` handles worker join — this runs on normal process exit from Ctrl+C via Rust's panic/drop machinery. The school-project scope does not require zero-downtime deploys.

If unsafe is acceptable for signal handling only:
```rust
use std::sync::atomic::{AtomicBool, Ordering};

static SHUTDOWN: AtomicBool = AtomicBool::new(false);

extern "C" fn handle_sigterm(_: libc::c_int) {
    SHUTDOWN.store(true, Ordering::Relaxed);
}
// register via libc::signal() — requires libc crate or raw FFI
```

**Recommendation:** Skip SIGTERM for v1. Document it as a known limitation. SIGINT (Ctrl+C) causes `TcpListener::incoming()` to return an error, breaking the loop, and Drop on ThreadPool runs. This is acceptable.

---

### 5. HTTP/1.1 Response Formatting

**Modules:** `std::fmt`, `std::io::Write`, `std::io::BufWriter`

The current code manually formats response lines. This is correct. The key additions for RFC compliance:

| Header | std API | Format |
|--------|---------|--------|
| `Date` | Existing `DateTime` in-tree | `Date: Thu, 01 Jan 1970 00:00:00 GMT\r\n` (RFC 7231 IMF-fixdate) |
| `Content-Length` | `file.metadata()?.len()` | `Content-Length: 1234\r\n` |
| `Content-Type` | `mime_for_path()` | `Content-Type: text/html; charset=utf-8\r\n` |
| `Connection` | literal string | `Connection: close\r\n` (correct for HTTP/1.1 without keep-alive) |
| `Server` | literal string | `Server: ptodd/0.1\r\n` (optional but useful) |

**Response struct design (no external deps):**
```rust
pub struct Response {
    pub status: u16,
    pub reason: &'static str,
    pub headers: Vec<(String, String)>,
    pub body: ResponseBody,
}

pub enum ResponseBody {
    Empty,
    Static(&'static str),         // for error pages
    File(std::fs::File, u64),      // (file handle, content-length)
}

impl Response {
    pub fn write_to(self, stream: TcpStream) -> std::io::Result<()> {
        use std::io::{BufWriter, Write};
        let mut w = BufWriter::new(stream);
        write!(w, "HTTP/1.1 {} {}\r\n", self.status, self.reason)?;
        for (k, v) in &self.headers {
            write!(w, "{}: {}\r\n", k, v)?;
        }
        write!(w, "\r\n")?;
        match self.body {
            ResponseBody::Empty => {}
            ResponseBody::Static(s) => { w.write_all(s.as_bytes())?; }
            ResponseBody::File(mut f, _) => { std::io::copy(&mut f, &mut w)?; }
        }
        w.flush()
    }
}
```

**IMF-fixdate for the `Date` header (RFC 7231 §7.1.1.1):**
The existing `DateTime` needs a new `Display` format for HTTP. HTTP requires:
`Thu, 01 Jan 1970 00:00:00 GMT` — this means the `DateTime` must expose weekday, and the existing struct does not store it. Weekday can be derived from `epoch_days % 7` (Unix epoch day 0 = Thursday).

```rust
// epoch_days % 7 → weekday index, where 0 = Thursday
const WEEKDAYS: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
let weekday = WEEKDAYS[(epoch_days % 7) as usize];
```

---

### 6. Request Parsing Hardening (Existing Bug Fixes)

**Modules:** `std::io::BufReader`, `std::io::BufRead`

The current `handle_connection` has several bugs the milestone must fix:

**Bug 1 — `unwrap()` on lines iterator panics on non-UTF-8 input:**
```rust
// Current (panics on binary/malformed requests):
.map(|result| result.unwrap())

// Fixed (returns Err on bad input, closes connection cleanly):
.lines()
.map(|r| r.map_err(|e| Error::Io(e)))
.collect::<Result<Vec<_>>>()?
```

**Bug 2 — No header size limit:**
```rust
// Add: limit lines read and total bytes read
.take(100)      // max 100 header lines
.take_while(|line| !line.is_empty())
```

**Bug 3 — Hardcoded file paths and routing:**
The `match http_request[0].as_str()` block must be replaced with a router that maps URL paths to filesystem paths under a configurable root directory.

---

### 7. Connection and I/O Timeouts

**Module:** `std::net::TcpStream`

Slow-loris and stalled clients will hold threads in the pool indefinitely without timeouts.

```rust
use std::time::Duration;
use std::net::TcpStream;

fn configure_stream(stream: &TcpStream) -> std::io::Result<()> {
    let timeout = Some(Duration::from_secs(30));
    stream.set_read_timeout(timeout)?;
    stream.set_write_timeout(timeout)?;
    Ok(())
}
```

`set_read_timeout` on the `TcpStream` causes `BufReader::lines()` to return `Err(WouldBlock)` / `TimedOut` after the timeout, unblocking the worker thread.

---

## Alternatives Considered

| Category | Recommended | Alternative | Why Not |
|----------|-------------|-------------|---------|
| MIME detection | `match` on extension | `mime` crate, `infer` crate | External deps are disallowed |
| Signal handling | Document limitation, skip for v1 | `signal-hook` crate, `ctrlc` crate | External deps are disallowed |
| File serving | `std::io::copy` | `fs::read_to_string` (current) | Binary files fail; full-file allocation |
| Response buffering | `BufWriter<TcpStream>` | Direct `TcpStream::write_all` | Many small writes degrade throughput |
| Path resolution | `Path::canonicalize` | Manual `..` stripping | Manual stripping is error-prone; canonicalize is authoritative |
| Async I/O | Synchronous blocking with pool | `tokio`, `async-std` | External deps are disallowed; also out of scope |

---

## What Cannot Be Done with std Only

| Capability | Missing Piece | Closest std Workaround |
|------------|--------------|------------------------|
| Graceful SIGTERM handling | No signal API in std (requires unsafe FFI or `signal-hook`) | Accept process kill; ThreadPool Drop runs on normal exit |
| MIME content sniffing | No magic-byte database | Extension-based match table (sufficient for web assets) |
| Keep-alive connection multiplexing | No HTTP framing beyond raw TCP | `Connection: close` on every response (correct but less efficient) |
| HTTP/2 | Protocol requires HPACK, multiplexing | Out of scope for this project |
| TLS | No TLS in std | Out of scope for this project |
| Range requests | Requires `Seek` + header parsing | `std::io::Seek` is available but adds significant complexity |
| Sendfile syscall | No platform-specific syscall wrappers | `std::io::copy` is portable and correct |

---

## Module Reference

| std module | APIs used | Purpose in project |
|------------|-----------|-------------------|
| `std::fs` | `File::open`, `File::metadata`, `metadata` | File reading, size for Content-Length |
| `std::io` | `BufReader`, `BufWriter`, `Read`, `Write`, `copy` | Buffered I/O on sockets and files |
| `std::path` | `Path`, `PathBuf`, `Path::canonicalize`, `Path::extension`, `Path::starts_with` | Path construction, traversal prevention, MIME detection |
| `std::net` | `TcpListener`, `TcpStream`, `set_read_timeout`, `set_write_timeout` | Network I/O, timeout configuration |
| `std::sync` | `Arc`, `Mutex`, `mpsc`, `atomic::AtomicBool` | Thread pool shared state, shutdown flag |
| `std::thread` | `thread::spawn`, `JoinHandle` | Worker threads |
| `std::time` | `SystemTime`, `Duration`, `UNIX_EPOCH` | Timeouts, Date header |
| `std::fmt` | `Display`, `write!` | Response header formatting |
| `std::env` | `std::env::args` | CLI args for server address, static root |

---

## Installation

No installation required. All APIs are part of Rust's standard library. The only `Cargo.toml` change needed is ensuring `log` v0.4.20 remains the sole dependency — no additions.

```toml
[dependencies]
log = { version = "0.4.20", features = ["std"] }
```

---

## Sources

All API documentation is from the Rust standard library (stable), applicable since Rust 1.56.0 (2021 edition, released 2021-10-21). Confidence is HIGH for all std API claims — these are stable, long-standing APIs with no deprecation or breaking changes in scope.

- `std::fs`: https://doc.rust-lang.org/std/fs/index.html
- `std::path`: https://doc.rust-lang.org/std/path/index.html
- `std::io`: https://doc.rust-lang.org/std/io/index.html
- `std::net`: https://doc.rust-lang.org/std/net/index.html
- RFC 9110 (HTTP Semantics): https://www.rfc-editor.org/rfc/rfc9110
- RFC 9112 (HTTP/1.1): https://www.rfc-editor.org/rfc/rfc9112
- RFC 7231 IMF-fixdate (Date header format): https://www.rfc-editor.org/rfc/rfc7231#section-7.1.1.1

---

*Research confidence: HIGH for all std API claims (stable, version-independent). LOW for signal handling — the "skip for v1" recommendation is an engineering judgment, not a technical constraint.*
