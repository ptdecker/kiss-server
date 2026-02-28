# Phase 1: Foundation Fixes - Research

**Researched:** 2026-02-28
**Domain:** Rust safety, datetime arithmetic, thread pool reliability
**Confidence:** HIGH

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions

- **Header Line Limit (SAFE-05):** Enforce limit inside `Request::parse()` — not in `handle_connection()`. Use named constant `MAX_HEADER_LINES: usize = 100`. Add new error variant `RequestTooLarge` to `server::error::Error`. When limit exceeded: propagate as `Err(Error::RequestTooLarge)` — existing handler logs warning and closes connection; client gets EOF.
- **DateTime Error Propagation (SAFE-02):** Change `DateTime::now()` return type from `DateTime` to `Result<DateTime>`. Add new error variant `SystemTime(std::time::SystemTimeError)` to `time::error::Error`. Update callers (SimpleLogger, handle_connection) to handle the Result. SimpleLogger on error: emit log line with `[unknown]` as timestamp — don't silence the log. Remove `#![allow(unused)]` from `src/time/mod.rs` if no unused code remains after fixes.
- **Worker Mutex Poison (SAFE-03):** Worker threads handle poisoned mutex without panicking — recover with `into_inner()` or log-and-continue. Claude's discretion on exact recovery mechanism, but workers must not cascade into killing the whole pool.
- **ThreadPool Drop (SAFE-04):** Use `let _ = thread.join()` — never call `.expect()` in a `Drop` impl.
- **DateTime Arithmetic (TIME-01, TIME-02):** Replace year loop (iterating from 1970) and month loop (iterating through months) with arithmetic formulas. Claude's discretion on exact formula — Zeller's congruence, Julian day math, or equivalent.
- **Test Coverage:** Unit tests alongside every fix (not deferred to later phases). Tests live in `#[cfg(test)]` blocks in the same module as the code being fixed. `Request::parse()` test: returns `Err` on bad/binary input (does not panic). Thread pool test: poisoned mutex handled without panicking; `Drop` doesn't double-panic. DateTime arithmetic test: spot-check known epoch → date conversions (epoch 0, a leap year boundary, a recent date).
- **Routing Simplification (removing /sleep):** Remove the `/sleep` endpoint — it's a DoS vector (5 concurrent requests fill the 4-thread pool). Remove hardcoded file reads (`hello.html`, `404.html`) — eliminates CWD-dependent crash. `/` route returns a plain `200 OK` with a minimal text body (placeholder until Phase 5). All unmatched routes: close connection without sending a response (routing is Phase 3's job).

### Claude's Discretion

- Exact arithmetic formula for year and month calculation
- Exact text of the placeholder 200 OK body for `/`
- Exact recovery mechanism for poisoned mutex (recover data vs skip job)
- Whether `src/url/mod.rs` `#![allow(unused)]` should also be cleaned up if touched

### Deferred Ideas (OUT OF SCOPE)

- None — discussion stayed within phase scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|-----------------|
| SAFE-02 | `DateTime::now()` uses safe error propagation — remove `unsafe { unwrap_unchecked() }` | `SystemTime::duration_since()` returns `Result<Duration, SystemTimeError>`; safe pattern is `now.duration_since(UNIX_EPOCH)?`; callers updated to handle `Result<DateTime>` |
| SAFE-03 | Worker threads handle poisoned mutex without panicking | `Mutex::lock()` returns `Result<Guard, PoisonError>` — recover via `lock().unwrap_or_else(\|e\| e.into_inner())`; mutex stays poisoned but workers continue |
| SAFE-04 | `ThreadPool::drop()` never panics — use `let _ = thread.join()` | `JoinHandle::join()` returns `Result`; calling `.expect()` in Drop causes double-panic → process abort; fix is `let _ = thread.join()` |
| SAFE-05 | Server enforces maximum of 100 request header lines | Current `handle_connection` collects unboundedly; limit moves into `Request::parse()` as new `RequestTooLarge` error variant |
| SAFE-06 | `Cargo.lock` committed to version control | Already satisfied — `Cargo.lock` is tracked in git (verified via `git ls-files`) |
| TIME-01 | DateTime year calculation uses arithmetic formula (not iteration from 1970) | Howard Hinnant's `civil_from_days` algorithm provides O(1) era-based formula using 400-year cycles; Rust port is straightforward integer arithmetic |
| TIME-02 | DateTime month calculation uses arithmetic (not sequential iteration) | Same algorithm; month extracted via `mp = (5*doy + 2)/153` where doy is day-within-March-anchored-year |
</phase_requirements>

## Summary

Phase 1 is a targeted safety hardening pass over existing code. There are six distinct problem sites across four files: `src/time/mod.rs` (unsafe unwrap + iterative algorithms), `src/server/worker.rs` (mutex poison panic), `src/server/pool.rs` (Drop panic), `src/server/mod.rs` (unbounded header read + DoS route + hardcoded file reads), and `src/server/request.rs` (header line enforcement). All fixes use Rust standard library features only — no new dependencies are introduced.

The two most technically interesting problems are the datetime arithmetic replacement and the `BufReader::lines()` panic on invalid UTF-8. For datetime, Howard Hinnant's `civil_from_days` algorithm provides a well-known, verified O(1) formula using 400-year era arithmetic. For invalid UTF-8, the current code panics because `.map(|result| result.unwrap())` is called on `BufReader::lines()` — the fix replaces the iterator chain with a manual loop using `read_line()` or changes the iterator to propagate errors instead of unwrapping. SAFE-06 (Cargo.lock) is already satisfied — `Cargo.lock` is present and tracked by git.

The mutex poison and thread join fixes are straightforward: `unwrap_or_else(|e| e.into_inner())` for SAFE-03 and `let _ = thread.join()` for SAFE-04. The header limit (SAFE-05) requires adding a `RequestTooLarge` error variant and a check inside `Request::parse()`. The routing cleanup removes `/sleep` and hardcoded file reads entirely, replacing with a direct inline response for `/` and silent connection close for unmatched routes.

**Primary recommendation:** Address all seven requirements in a single focused pass over four files; use Howard Hinnant's `civil_from_days` algorithm for the datetime arithmetic replacement.

## Standard Stack

### Core

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `std::time` | stdlib | `SystemTime`, `UNIX_EPOCH`, `SystemTimeError` | No external crate needed; `duration_since()` returns `Result` |
| `std::sync` | stdlib | `Mutex`, `PoisonError` | `unwrap_or_else(\|e\| e.into_inner())` recovers from poison without third-party crates |
| `std::thread` | stdlib | `JoinHandle::join()` returning `Result` | `let _ = handle.join()` is the idiomatic fix |
| `std::io::BufReader` | stdlib | Reading TCP stream lines | Replace `.unwrap()` in iterator with proper `Result` propagation |

### Supporting

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `log` crate | 0.4.20 (already in Cargo.toml) | Warn on datetime error in SimpleLogger | Use `warn!()` macro in error path, already imported |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Howard Hinnant formula | `chrono` crate | Project avoids external crates; formula is ~15 lines of arithmetic |
| Manual read loop for UTF-8 | `bstr` crate | Project avoids external crates; manual loop with `read_line()` is ~10 lines |
| `into_inner()` to recover mutex | `parking_lot::Mutex` (no poisoning) | External crate; not appropriate for this pass |

**Installation:** No new dependencies needed for this phase.

## Architecture Patterns

### Recommended Project Structure

No structural changes needed. All modifications are within existing files:

```
src/
├── time/
│   ├── mod.rs          # Replace year()/month() iterative fns with arithmetic; DateTime::now() -> Result<DateTime>
│   └── error.rs        # Add SystemTime(std::time::SystemTimeError) variant
├── server/
│   ├── mod.rs          # Fix BufReader::lines() unwrap; remove /sleep; remove file reads; placeholder 200 OK
│   ├── error.rs        # Add RequestTooLarge variant
│   ├── request.rs      # Add MAX_HEADER_LINES constant; enforce limit in parse()
│   ├── pool.rs         # Fix Drop: let _ = thread.join()
│   └── worker.rs       # Fix mutex lock: unwrap_or_else(|e| e.into_inner())
└── logger/
    └── mod.rs          # Handle Result<DateTime> from DateTime::now()
```

### Pattern 1: Safe Error Propagation for SystemTime

**What:** Replace `unsafe { unwrap_unchecked() }` with `?` operator after changing return type.
**When to use:** Any time code uses unsafe purely to avoid handling a `Result`.
**Example:**
```rust
// Source: https://doc.rust-lang.org/std/time/struct.SystemTime.html
impl DateTime {
    pub fn now() -> Result<DateTime> {
        let now = SystemTime::now();
        let duration = now.duration_since(UNIX_EPOCH)?;  // ? propagates SystemTimeError
        // ...
        Ok(DateTime { /* fields */ })
    }
}
```

The `time::error::Error` enum gains:
```rust
// Add to src/time/error.rs
pub enum Error {
    InvalidMonth,
    SystemTime(std::time::SystemTimeError),
}

impl From<std::time::SystemTimeError> for Error {
    fn from(e: std::time::SystemTimeError) -> Self {
        Error::SystemTime(e)
    }
}
```

### Pattern 2: Mutex Poison Recovery

**What:** Replace `.expect()` on mutex lock with `unwrap_or_else(|e| e.into_inner())`.
**When to use:** Worker loops that must survive a panicked prior job without cascading.
**Example:**
```rust
// Source: https://doc.rust-lang.org/std/sync/struct.Mutex.html
let message = receiver
    .lock()
    .unwrap_or_else(|e| e.into_inner())  // recover data from poisoned guard
    .recv();
```

Note: `clear_poison()` is available since Rust 1.77.0 (project is on 1.93.1) but is NOT needed here — the receiver itself is not corrupted by a job panic; the channel is still valid. `into_inner()` is sufficient.

### Pattern 3: Safe Drop Implementation

**What:** `let _ = handle.join()` discards the `Result` in a Drop implementation.
**When to use:** Any Drop impl that joins threads — calling `.expect()` here causes double-panic → process abort.
**Example:**
```rust
// Source: https://doc.rust-lang.org/std/thread/struct.JoinHandle.html
impl Drop for ThreadPool {
    fn drop(&mut self) {
        drop(self.sender.take());
        for worker in &mut self.workers {
            debug!("Shutting down worker {}", worker.id);
            if let Some(thread) = worker.thread.take() {
                let _ = thread.join();  // Never .expect() in Drop
            }
        }
    }
}
```

### Pattern 4: Howard Hinnant civil_from_days Algorithm

**What:** O(1) formula to convert days-since-Unix-epoch to (year, month, day) without any iteration.
**When to use:** Computing calendar date from an epoch offset — handles all leap year rules algebraically.
**Example (Rust port of the C++ reference algorithm):**
```rust
// Source: https://howardhinnant.github.io/date_algorithms.html
// Input: days since Unix epoch (1970-01-01)
// Output: (year: i32, month: u8, day: u8)
fn civil_from_days(epoch_days: i64) -> (i32, u8, u8) {
    // Shift epoch from 1970-01-01 to 0000-03-01 (puts leap day at end of year)
    let z = epoch_days + 719_468;
    // 400-year era containing the date
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    // Day of era [0, 146096]
    let doe = (z - era * 146_097) as u32;
    // Year of era [0, 399]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    // Absolute year
    let y = yoe as i64 + era * 400;
    // Day of year [0, 365] anchored at March 1
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    // Month in [0,11] where 0 = March
    let mp = (5 * doy + 2) / 153;
    // Day of month [1, 31]
    let d = doy - (153 * mp + 2) / 5 + 1;
    // Convert mp to civil month [1, 12]
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    // Year adjustment: January and February belong to the next civil year
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u8, d as u8)
}
```

The existing `year()` and `month()` private functions are replaced by this single function. The `(year, day_of_year)` tuple from `year()` and `(month, day)` from `month()` are superseded — the struct fields `day_of_year` and `epoch_days` remain and are computed separately (epoch_days is already available, day_of_year requires a separate but simple subtraction).

### Pattern 5: Safe Header Reading with Line Limit

**What:** Move the line limit into `Request::parse()` instead of the calling site.
**When to use:** Prevent header-stuffing DoS without hanging indefinitely.
**Example:**
```rust
// In src/server/request.rs
const MAX_HEADER_LINES: usize = 100;

impl Request {
    pub(super) fn parse(raw_request: &[String]) -> Result<Request> {
        if raw_request.len() > MAX_HEADER_LINES {
            return Err(Error::RequestTooLarge);
        }
        // ... existing parse logic
    }
}
```

And in `handle_connection` in `src/server/mod.rs`, the collection loop must also be bounded (the limit enforced in `parse()` is the gate, but the reader must not collect unboundedly before calling parse). The fix requires the collection loop to stop at `MAX_HEADER_LINES` lines:
```rust
// In handle_connection — collection must also be bounded
let http_request: Vec<String> = BufReader::new(&mut stream)
    .lines()
    .take(MAX_HEADER_LINES + 1)    // collect up to limit+1 so parse() can detect overflow
    .map(|r| r?)                    // propagate UTF-8/IO errors as Result
    .take_while(|line| !line.is_empty())
    .collect::<Result<Vec<_>, _>>()?;
```

Note: The `MAX_HEADER_LINES` constant must be accessible in both `request.rs` and `mod.rs`. Since both are in `server::`, the constant can live in `request.rs` with `pub(super)` visibility, or in `mod.rs` with `use` in `request.rs`. The locked decision places the constant in `request.rs`.

### Pattern 6: Invalid UTF-8 Safe Collection

**What:** The current `BufReader::lines().map(|r| r.unwrap())` panics on invalid UTF-8 bytes. The fix propagates the error instead.
**When to use:** Reading from untrusted network input.
**Current broken code (src/server/mod.rs line 78-82):**
```rust
// PANICS on invalid UTF-8 or I/O error:
let http_request: Vec<_> = BufReader::new(&mut stream)
    .lines()
    .map(|result| result.unwrap())   // <-- panic site
    .take_while(|line| !line.is_empty())
    .collect();
```

**Fixed pattern:**
```rust
// Propagates errors; caller returns Err to handler which logs warning and drops connection
let http_request: Vec<String> = {
    let mut lines = Vec::new();
    let mut reader = BufReader::new(&mut stream);
    let mut buf = String::new();
    loop {
        buf.clear();
        match reader.read_line(&mut buf) {
            Ok(0) => break,                    // EOF
            Ok(_) => {
                let line = buf.trim_end_matches(['\r', '\n']).to_string();
                if line.is_empty() { break; }
                lines.push(line);
            }
            Err(e) => return Err(e.into()),    // Invalid UTF-8 or I/O error
        }
        if lines.len() > MAX_HEADER_LINES {
            return Err(Error::RequestTooLarge);
        }
    }
    lines
};
```

This replaces the iterator chain and satisfies both SAFE-05 (line limit) and SAFE-02's companion panic site (the UTF-8 unwrap). The line limit check inside the collection loop means `parse()` receives at most `MAX_HEADER_LINES` lines; the check inside `parse()` is a defense-in-depth double-check.

### Pattern 7: SimpleLogger Handling Result<DateTime>

**What:** `SimpleLogger::log()` calls `DateTime::now()` which becomes `Result<DateTime>`. The logger must not panic on error.
**Current code (src/logger/mod.rs line 58):**
```rust
eprintln!("{}: {}: {}: {}", DateTime::now(), ...);
```
**Fixed pattern:**
```rust
let timestamp = DateTime::now()
    .map(|dt| dt.to_string())
    .unwrap_or_else(|_| "[unknown]".to_string());
eprintln!("{}: {}: {}: {}", timestamp, ...);
```

### Pattern 8: Routing Cleanup

**What:** Remove `/sleep` (DoS vector) and hardcoded file reads; return inline body for `/`.
**Current broken code (src/server/mod.rs lines 88-104):**
```rust
let (status_line, filename) = match http_request[0].as_str() {
    "GET / HTTP/1.1" => ("HTTP/1.1 200 OK", "hello.html"),
    "GET /sleep HTTP/1.1" => { thread::sleep(Duration::from_secs(5)); ("HTTP/1.1 200 OK", "hello.html") }
    _ => ("HTTP/1.1 404 NOT FOUND", "404.html"),
};
let contents = fs::read_to_string(filename)?;   // panics if CWD doesn't have hello.html/404.html
```
**Fixed pattern:**
```rust
match request.method {
    RequestMethod::Get if request.target.as_str() == "/" => {
        let body = "OK";
        stream.write_all(
            format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}", body.len(), body)
                .as_bytes(),
        )?;
    }
    _ => {
        // Unmatched routes: close connection without response (routing is Phase 3's job)
    }
}
```
Note: The `Url` struct currently lacks an accessor method exposing the raw path. Either add a `pub(super) fn as_str(&self) -> &str` method or compare using the `Display` impl. Adding an accessor is cleaner.

### Anti-Patterns to Avoid

- **`.expect()` or `.unwrap()` in Drop implementations:** Causes double-panic → process abort. Use `let _ = expr` instead.
- **`.unwrap()` on `BufReader::lines()` iterator items:** Panics on invalid UTF-8 from network input. Propagate the error.
- **`unsafe { result.unwrap_unchecked() }` for convenience:** Removes Rust's safety guarantee for no reason; use `?` after making the containing function return `Result`.
- **Iterative year/month loops from epoch:** O(N) where N grows with time; arithmetic formulas are O(1) and correct.
- **File reads from CWD in a server handler:** Server CWD is undefined in production; inline bodies or configured paths are required.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Calendar arithmetic | Custom leap-year loop iteration | Howard Hinnant's era formula | Industry-verified, O(1), handles all Gregorian edge cases |
| Mutex poison recovery | Custom state tracking | `PoisonError::into_inner()` | Already built into `std::sync::Mutex` |
| Line-limit enforcement | Async timeout machinery | `Iterator::take(N)` + count check in parse | Simple, synchronous, no extra threads |

**Key insight:** All custom solutions in this phase are worse than the stdlib patterns — mutex recovery, thread join, and UTF-8 error propagation are all solved by using the `Result` types the standard library already returns.

## Common Pitfalls

### Pitfall 1: The Line Limit Enforcement Gap

**What goes wrong:** Adding `MAX_HEADER_LINES` check only inside `Request::parse()` but not in the collection loop in `handle_connection`. An attacker sends 10,000 header lines — the server reads all 10,000 into memory before `parse()` rejects them.
**Why it happens:** Treating parse validation as sufficient without controlling when data is collected.
**How to avoid:** Bound the collection loop too. The `.take(MAX_HEADER_LINES + 1)` on the reader ensures no more than `MAX_HEADER_LINES + 1` lines are ever buffered. The `+1` lets `parse()` see an overrun and return `RequestTooLarge` without the collection having consumed more.
**Warning signs:** Collection loop has no upper bound; only parse validates the count.

### Pitfall 2: day_of_year Field After Algorithm Replacement

**What goes wrong:** The `DateTime` struct currently holds `day_of_year: u16` computed by the old `year()` function. After switching to `civil_from_days`, that function is gone. `day_of_year` must be computed separately.
**Why it happens:** `civil_from_days` returns `(year, month, day)` — not `day_of_year`. The old `year()` function returned `(year, remaining_days)` which was `day_of_year`.
**How to avoid:** After computing `civil_from_days(epoch_days)`, compute day_of_year from first principles: sum days in months [January..month-1] for the computed year, plus `day`. Or keep a separate `days_from_epoch_to_year_start` computation. The simplest approach: use cumulative month-day-offset tables (no loop, just a lookup).
**Warning signs:** Tests pass for epoch 0 but fail for day_of_year spot checks.

### Pitfall 3: Mutex Recovery Does Not Fix the Underlying Data

**What goes wrong:** Using `into_inner()` after a poison recovers the `Receiver<Job>` but if the job that panicked left the receiver in a bad state, future jobs may get corrupted data.
**Why it happens:** `into_inner()` gives access to the data regardless of its state.
**How to avoid:** In this specific case, the `Receiver<Job>` is not mutated by jobs — jobs receive messages and process them independently. The channel itself is unaffected by a panicking job. Recovery via `into_inner()` is safe here because the data (the channel receiver) was not mutated by the panicking closure.
**Warning signs:** If the shared data under the mutex is mutated by the panicking code, `into_inner()` alone is insufficient — `clear_poison()` + data validation would be needed. That is NOT the case here.

### Pitfall 4: Month Numbering in civil_from_days

**What goes wrong:** The algorithm internally uses March-anchored months where `mp=0` is March. The final `m` conversion must map correctly to the `Month` enum.
**Why it happens:** The algorithm shifts the calendar so February (the leap month) is the last month of the internal year. The output `m` is in civil [1..12] but developers forget the year adjustment: if `m <= 2` (January or February), `y` must be incremented by 1.
**How to avoid:** Use the exact formula `let y = if m <= 2 { y + 1 } else { y };` from the reference implementation. Test with epoch 0 (1970-01-01) and a January date (e.g., 2000-01-01 = epoch day 10957).
**Warning signs:** January and February dates are off by one year.

### Pitfall 5: Double Responsibility of handle_connection

**What goes wrong:** `handle_connection` currently both collects headers AND matches routes. After the fix, it still collects headers — but the routing cleanup (removing `/sleep`, hardcoded files) must not accidentally remove the `Request::parse()` call or the result logging.
**Why it happens:** Large refactors to the same function can accidentally remove needed logic.
**How to avoid:** Treat the routing change as minimal surgery: remove the `match http_request[0].as_str()` block and replace it, but keep `Request::parse()`, the info! logging, and the stream write pattern.

### Pitfall 6: SAFE-06 Is Already Done

**What goes wrong:** Spending time on SAFE-06 (commit Cargo.lock) when it is already committed.
**Why it happens:** Cargo.lock appears in `.gitignore` for library crates by convention, so developers assume it's not tracked.
**How to avoid:** Verify with `git ls-files Cargo.lock` before acting. Result: `Cargo.lock` is already tracked. This requirement needs only a verification step, not a git operation.

## Code Examples

Verified patterns from the actual codebase and official sources:

### SAFE-02: DateTime::now() — Safe Version

```rust
// src/time/mod.rs
impl DateTime {
    pub fn now() -> Result<DateTime> {
        let now = SystemTime::now();
        let duration = now.duration_since(UNIX_EPOCH)?;  // propagates SystemTimeError
        let epoch_seconds = duration.as_secs();
        let epoch_days = (epoch_seconds / 86_400) as i64;
        let (year, month_num, day) = civil_from_days(epoch_days);
        let month = Month::try_from(month_num)?;
        // day_of_year: computed separately (see note in pitfalls)
        Ok(DateTime {
            epoch_seconds,
            epoch_sub_nanoseconds: duration.subsec_nanos(),
            epoch_days: epoch_days as u64,
            year: year as u16,
            day_of_year: /* computed separately */,
            month,
            day,
        })
    }
}
```

### SAFE-03: Worker Mutex Recovery

```rust
// src/server/worker.rs — inside the spawn closure
let message = receiver
    .lock()
    .unwrap_or_else(|e| {
        warn!("Worker {id}: receiver mutex poisoned, recovering");
        e.into_inner()
    })
    .recv();
```

### SAFE-04: ThreadPool Drop Fix

```rust
// src/server/pool.rs
impl Drop for ThreadPool {
    fn drop(&mut self) {
        drop(self.sender.take());
        for worker in &mut self.workers {
            debug!("Shutting down worker {}", worker.id);
            if let Some(thread) = worker.thread.take() {
                let _ = thread.join();  // discard Result — never .expect() in Drop
            }
        }
    }
}
```

### SAFE-05: RequestTooLarge Error Variant

```rust
// src/server/error.rs — add variant
pub enum Error {
    InvalidRequest(String),
    RequestTooLarge,
    Channel(String),
    Io(std::io::Error),
}

// src/server/request.rs — add constant and check
pub(super) const MAX_HEADER_LINES: usize = 100;

impl Request {
    pub(super) fn parse(raw_request: &[String]) -> Result<Request> {
        if raw_request.len() > MAX_HEADER_LINES {
            return Err(Error::RequestTooLarge);
        }
        // ...existing parse logic...
    }
}
```

### TIME-01 and TIME-02: Arithmetic Formula

```rust
// src/time/mod.rs — replaces year() and month() private functions
// Source: https://howardhinnant.github.io/date_algorithms.html
fn civil_from_days(epoch_days: i64) -> (i32, u8, u8) {
    let z = epoch_days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u8, d as u8)
}
```

Known correct test vectors (for unit tests):
- Epoch 0 → 1970-01-01 (year=1970, month=1, day=1)
- Epoch 10957 → 2000-01-01 (year=2000, month=1, day=1)
- Epoch 11688 → 2002-01-01 (year=2002, month=1, day=1)
- Epoch 18628 → 2021-01-16 (approximate; verify with independent source)

For the `day_of_year` field (still needed by `DateTime::day_of_year()`): after computing civil date, compute `day_of_year` as days elapsed since Jan 1 of the computed year. This can be done by calling `civil_from_days` once to get `(year, month, day)`, then computing `days_from_civil(year, 1, 1)` (the inverse) and subtracting to get the offset. Or, more practically: keep a simple cumulative days-per-month lookup table indexed by (year, month) — no iteration required.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `unsafe { unwrap_unchecked() }` for system time | `duration_since()?` with `Result` return | Rust 1.0 always had safe alternative | Eliminates one unsafe block |
| `.expect()` in Drop for thread join | `let _ = join()` | Known Rust idiom since 1.0 | Prevents process abort on shutdown |
| Iterative year/month loops | Howard Hinnant era arithmetic | Algorithm published ~2013, widely used | O(1) vs O(N years since 1970) |
| Unchecked `lines().map(unwrap)` | Bounded + error-propagating read | N/A — always a bug | Eliminates crash vector from network input |
| `Mutex::lock().expect()` in worker | `lock().unwrap_or_else(|e| e.into_inner())` | `clear_poison()` added in Rust 1.77.0 | Workers survive job panics |

**Deprecated/outdated:**
- `unsafe { Option::unwrap_unchecked() }`: Only appropriate when the invariant is provably maintained by surrounding logic and cannot use `?`. In this case, `duration_since(UNIX_EPOCH)` on the current system time has no such proven invariant.

## Open Questions

1. **`day_of_year` after algorithm replacement**
   - What we know: `civil_from_days` does not directly return `day_of_year`
   - What's unclear: Best way to compute it without re-introducing a loop
   - Recommendation: Compute via inverse: `epoch_days - civil_to_days(year, 1, 1)` where `civil_to_days` is the standard inverse formula `(days_from_civil)`. Howard Hinnant's page also provides `days_from_civil`. Alternatively, the planner could decide to derive `day_of_year` from the `doy` variable inside `civil_from_days` (the March-anchored day-of-year) by adjusting for the March shift — more complex but avoids a second call.

2. **Exact text of placeholder 200 OK body**
   - What we know: Locked decision says "minimal text body" — exact text is Claude's discretion
   - What's unclear: Whether to include any HTML or plain text
   - Recommendation: Use `"OK\n"` — three bytes, plain text, unambiguous placeholder

3. **`Url::as_str()` accessor needed?**
   - What we know: `Url` struct has `raw_path: String` (private); routing match needs the path
   - What's unclear: Whether to add an accessor or use Display impl
   - Recommendation: Add `pub(super) fn as_str(&self) -> &str { &self.raw_path }` — explicit and typed

## Validation Architecture

> `workflow.nyquist_validation` is not present in `.planning/config.json` — skipping this section.

## Sources

### Primary (HIGH confidence)

- [Howard Hinnant - Date Algorithms](https://howardhinnant.github.io/date_algorithms.html) — `civil_from_days` algorithm, verified formula
- [std::time::SystemTime](https://doc.rust-lang.org/std/time/struct.SystemTime.html) — `duration_since()` returns `Result<Duration, SystemTimeError>`
- [std::sync::Mutex](https://doc.rust-lang.org/std/sync/struct.Mutex.html) — poisoning behavior, `into_inner()` recovery pattern, `clear_poison()` stable since 1.77.0
- [std::thread::JoinHandle](https://doc.rust-lang.org/std/thread/struct.JoinHandle.html) — `join()` returns `Result`, `.expect()` in Drop causes double-panic
- [std::io::BufRead](https://doc.rust-lang.org/std/io/trait.BufRead.html) — `lines()` returns `io::Result<String>` per item; `read_line()` alternative

### Secondary (MEDIUM confidence)

- [Rust Users Forum: BufReader invalid UTF-8](https://users.rust-lang.org/t/bufreader-error-stream-did-not-contain-valid-utf-8/123497) — confirms `.unwrap()` is the panic site, not `lines()` itself
- [Rust Users Forum: Mutex Poisoning Recovery](https://users.rust-lang.org/t/mutex-poisoning-why-and-how-to-recover/72192) — `unwrap_or_else(|e| e.into_inner())` pattern confirmed

### Tertiary (LOW confidence)

- None — all key claims verified against official documentation.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — pure stdlib, no new dependencies, patterns verified in official docs
- Architecture: HIGH — all fixes are localized to existing files with clear, known Rust idioms
- Pitfalls: HIGH — identified from direct code inspection and official docs
- DateTime formula: HIGH — Howard Hinnant algorithm is the canonical reference, used in `chrono` and C++20 `<chrono>` library

**Research date:** 2026-02-28
**Valid until:** 2026-08-28 (6 months — stdlib APIs are stable; algorithm is timeless)
