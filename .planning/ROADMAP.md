# Roadmap: ptodd

## Overview

The server already has TCP acceptance, a thread pool, request parsing, and time utilities — but
crashes on bad input, reads files incorrectly, has hardcoded paths, and has no defense against
path traversal. The roadmap fixes the broken foundation first, then layers the Response struct,
Handler/Context/Router abstractions, URL safety, and static file serving on top in strict
dependency order. Each phase leaves the server in a testable, working state.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [x] **Phase 1: Foundation Fixes** - Eliminate crashes, unsafe code, and silent DoS vectors before any new code is added
- [x] **Phase 2: Response and HTTP Compliance** - Build the Response struct with mandatory HTTP/1.1 headers; all responses use CRLF and correct byte serialization (completed 2026-03-02)
- [ ] **Phase 3: Handler, Context, and Router** - Define the Context/Handler/Router abstractions and wire them into the server; 404 and 500 responses dispatched through the pipeline
- [ ] **Phase 4: URL Path Safety** - Add path() and decoded_path() to Url; enforce dot-dot rejection and canonicalize-based traversal prevention
- [ ] **Phase 5: Static File Serving** - Implement StaticFileHandler with MIME detection, binary-safe reads, configurable root, and HEAD support

## Phase Details

### Phase 1: Foundation Fixes
**Goal**: The server handles bad input without panicking, resists simple DoS attacks, and has no unsafe code in the datetime utilities
**Depends on**: Nothing (first phase)
**Requirements**: SAFE-02, SAFE-03, SAFE-04, SAFE-05, SAFE-06, TIME-01, TIME-02
**Success Criteria** (what must be TRUE):
  1. Sending binary or invalid UTF-8 bytes at the server does not crash the worker thread
  2. A poisoned mutex in the worker queue does not cascade into killing all workers
  3. The server process does not double-panic when the thread pool is dropped during shutdown
  4. The server stops reading request headers after 100 lines, without hanging indefinitely
  5. DateTime year and month are computed via arithmetic formula, not iterative loops
**Plans**: 2 plans

Plans:
- [x] 01-01-PLAN.md — DateTime arithmetic + safe error propagation (SAFE-02, TIME-01, TIME-02)
- [x] 01-02-PLAN.md — Server safety fixes: mutex poison, Drop, header limit, routing cleanup (SAFE-03, SAFE-04, SAFE-05, SAFE-06)

### Phase 2: Response and HTTP Compliance
**Goal**: Every response the server sends is a valid HTTP/1.1 message: CRLF-terminated lines, correct mandatory headers, and byte-accurate Content-Length
**Depends on**: Phase 1
**Requirements**: SAFE-01, HTTP-01, HTTP-02, HTTP-03, HTTP-04, HTTP-05, HTTP-06, TIME-03
**Success Criteria** (what must be TRUE):
  1. A response viewed with a raw TCP client shows CRLF line endings and a blank line before the body
  2. Every response includes Content-Type, Content-Length, Date, and Connection: close headers
  3. The Date header value is formatted as IMF-fixdate (e.g., "Sat, 28 Feb 2026 00:00:00 GMT")
  4. Sending a malformed HTTP request returns a 400 Bad Request response, not a crash or hang
  5. Content-Length matches the actual byte length of the response body for binary content
**Plans**: 3 plans

Plans:
- [ ] 02-01-PLAN.md — DateTime::to_imf_fixdate() for HTTP Date header (TIME-03)
- [ ] 02-02-PLAN.md — Response struct: value-chaining builder + write_to (HTTP-01, HTTP-02, HTTP-04, HTTP-05)
- [ ] 02-03-PLAN.md — handle_connection refactor: wire Response + 400/431 error responses (SAFE-01, HTTP-01, HTTP-02, HTTP-03, HTTP-06)

### Phase 3: Handler, Context, and Router
**Goal**: The server dispatches requests through a Router to typed Handlers via a shared Context struct; unmatched routes return 404 and handler errors return 500
**Depends on**: Phase 2
**Requirements**: ROUT-01, ROUT-02, ROUT-03, ROUT-04, HTTP-07, HTTP-08
**Success Criteria** (what must be TRUE):
  1. A request to an unregistered path receives a 404 Not Found response from the NotFoundHandler fallback
  2. A handler that returns an error causes the server to respond 500 Internal Server Error without crashing
  3. Multiple handlers can be registered and the router dispatches to the first match in registration order
  4. The Context struct holds both Request and Response as mutable pipeline state accessible to the handler
**Plans**: 3 plans

Plans:
- [ ] 03-01-PLAN.md — Visibility promotion (pub Request/RequestMethod/PartialEq) + Response::add_header (ROUT-01, ROUT-02, ROUT-04)
- [ ] 03-02-PLAN.md — Handler trait, Context struct, Router with NotFoundHandler fallback (ROUT-01, ROUT-02, ROUT-03, ROUT-04, HTTP-07)
- [ ] 03-03-PLAN.md — RootHandler, handle_connection refactor, Server::with_router, main.rs wiring (ROUT-01, ROUT-02, ROUT-03, ROUT-04, HTTP-07, HTTP-08)

### Phase 4: URL Path Safety
**Goal**: The server decodes percent-encoded paths before routing and rejects all requests whose paths escape the configured root
**Depends on**: Phase 3
**Requirements**: PATH-01, PATH-02, PATH-03
**Success Criteria** (what must be TRUE):
  1. A request for a percent-encoded path (e.g., /my%20file.html) resolves to the correct decoded filename
  2. A request containing a literal ".." path component returns 404, not a file from outside the root
  3. A path that after canonicalization resolves outside the server root returns 404
**Plans**: TBD

### Phase 5: Static File Serving
**Goal**: The server correctly serves any file under a configurable root directory — binary-safe, with accurate MIME types, configurable via CLI argument, and responding correctly to HEAD requests
**Depends on**: Phase 4
**Requirements**: FILE-01, FILE-02, FILE-03, FILE-04, FILE-05
**Success Criteria** (what must be TRUE):
  1. Requesting an HTML, CSS, JS, WASM, or image file returns the correct Content-Type header for that file type
  2. A binary file (WASM, PNG) is served with identical bytes to what is on disk, with no corruption
  3. The server root directory is set via a CLI argument at startup with no hardcoded paths in the binary
  4. A HEAD request returns all headers (Content-Type, Content-Length, Date) with no body
  5. A request for a file that does not exist returns 404 Not Found
**Plans**: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → 4 → 5

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Foundation Fixes | 2/2 | Complete   | 2026-03-01 |
| 2. Response and HTTP Compliance | 3/3 | Complete   | 2026-03-02 |
| 3. Handler, Context, and Router | 2/3 | In Progress|  |
| 4. URL Path Safety | 0/TBD | Not started | - |
| 5. Static File Serving | 0/TBD | Not started | - |
