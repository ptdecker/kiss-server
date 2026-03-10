---
status: resolved
phase: 05-static-file-serving
source: [05-01-SUMMARY.md, 05-02-SUMMARY.md, 05-03-SUMMARY.md]
started: 2026-03-09T00:00:00Z
updated: 2026-03-10T00:00:00Z
---

## Current Test

[testing complete]

## Tests

### 1. Cold Start Smoke Test
expected: Kill any running server. Start the server from scratch with `cargo run -- --root .`. Server boots without errors, prints listening address, and a request to GET / returns 200 OK.
result: pass
notes: Server runs on localhost:6502 (DEFAULT_ADDR). Test instructions had wrong port (7878) — server started correctly.

### 2. Serve a Static File
expected: With server running (`cargo run -- --root .`), request `curl -i http://localhost:6502/hello.html`. Response is 200 OK with Content-Type: text/html and the file's content ("Hello") in the body.
result: pass

### 3. MIME Type Detection
expected: Request a non-HTML file extension (create a test file or check with a .txt file). The Content-Type header matches the extension — e.g., text/plain for .txt, text/css for .css, application/javascript for .js, image/png for .png. Unknown extensions get application/octet-stream.
result: pass

### 4. HEAD Request — Headers Only
expected: `curl -I http://localhost:6502/hello.html`. Response includes 200 OK status, Content-Type, Content-Length, Date, Connection headers — but NO body bytes. Content-Length reflects the file size.
result: pass

### 5. Missing File Returns 404
expected: `curl -i http://localhost:6502/nonexistent.txt`. Response is 404 Not Found with appropriate headers. No server crash.
result: pass

### 6. Path Traversal Rejected
expected: `curl -i --path-as-is 'http://localhost:6502/%2E%2E/etc/passwd'`. Response is 404 Not Found. The server does NOT return /etc/passwd or any file outside the root directory.
result: issue
reported: "returned the correct 404 Not Found but the error message was not followed by a carriage return as it is for other errors"
severity: minor

### 7. --root Controls Served Directory
expected: Create a temp directory with a test file, then start server with `cargo run -- --root /path/to/tempdir`. Requesting that file by name returns its content. Files in the project root are NOT served (they're outside the configured root).
result: pass

### 8. GET / Still Works with Static Serving
expected: With static file serving active (`cargo run -- --root .`), `curl -i http://localhost:6502/` still returns 200 OK from the RootHandler (not a file lookup). The Router's registered route takes priority over the StaticFileHandler fallback.
result: pass

## Summary

total: 8
passed: 7
issues: 1
pending: 0
skipped: 0

## Gaps

- truth: "All 404 responses have a consistent body format (text followed by newline)"
  status: resolved
  reason: "User reported: returned the correct 404 Not Found but the error message was not followed by a carriage return as it is for other errors"
  severity: minor
  test: 6
  root_cause: "NotFoundHandler in router.rs:90 uses body b\"Not Found\" (no newline); not_found() helper in handlers/mod.rs:49 uses b\"Not Found\\n\" (with newline). Path traversal hits the router guard → NotFoundHandler (no newline). StaticFileHandler 404s use not_found() helper (with newline)."
  artifacts:
    - path: "src/server/router.rs"
      issue: "NotFoundHandler body missing trailing newline (line 90)"
  missing:
    - "Add trailing \\n to NotFoundHandler body in router.rs to match not_found() helper"
  debug_session: ""
