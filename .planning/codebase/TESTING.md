# Testing Patterns

**Analysis Date:** 2026-02-28

## Test Framework

**Runner:**
- Rust built-in test framework (stable)
- Tests run via `cargo test`
- No external test runner crate dependency

**Assertion Library:**
- Standard `assert!`, `assert_eq!`, `assert_ne!` macros
- No external assertion library; uses Rust standard assertions

**Run Commands:**
```bash
cargo test              # Run all tests
cargo test -- --test-threads=1  # Run tests sequentially
cargo test -- --nocapture  # Show output from passing tests
cargo test --release   # Run tests in release mode
```

## Test File Organization

**Location:**
- Tests are co-located with source code using `#[cfg(test)]` modules
- Test modules appear at the end of source files following implementation

**Naming:**
- Test functions prefixed with `test_`: `#[test] fn hex_byte()`, `#[test] fn encode()`, `#[test] fn decode()`, `#[test] fn leap_year()`
- Test modules named `tests`

**Structure:**
```
src/
├── url/
│   └── mod.rs          # Contains tests module at end
├── time/
│   └── mod.rs          # Contains tests module at end
└── [other modules]     # No tests currently
```

## Test Structure

**Suite Organization:**
```rust
// src/url/mod.rs - Example test organization
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_byte() {
        const TEST_CHARS: [(u8, char); 22] = [
            // Test data setup inline
        ];
        for (value, char) in TEST_CHARS {
            assert_eq!(value, hex_char_to_byte(char).unwrap());
        }
    }

    #[test]
    fn encode() {
        assert_eq!("%65", pct_encode('\u{0065}'));
        assert_eq!("%C3%A9", pct_encode('\u{00E9}'));
    }
}
```

**Patterns:**
- No setup/teardown fixtures; tests are self-contained
- Test data defined inline within test functions using `const` arrays
- Iterative test patterns for comprehensive coverage
- Each assertion represents a test case variation

## Mocking

**Framework:** No mocking framework present

**Patterns:**
- Unit tests focus on pure functions with no external dependencies
- Tests for `hex_char_to_byte()`, `pct_encode()`, `pct_decode()` operate on pure logic
- No mock objects needed due to functional programming style

**What to Mock:**
- Not applicable; tests focus on functions without side effects
- Thread pool and server components lack tests, so no mocking patterns established

**What NOT to Mock:**
- Pure utility functions tested directly without mocks
- Results of encoding/decoding operations tested for correctness

## Fixtures and Factories

**Test Data:**
```rust
// src/url/mod.rs - Inline test data pattern
#[test]
fn hex_byte() {
    const TEST_CHARS: [(u8, char); 22] = [
        (0, '0'),
        (1, '1'),
        (2, '2'),
        // ... complete list of test values
        (15, 'F'),
    ];
    for (value, char) in TEST_CHARS {
        assert_eq!(value, hex_char_to_byte(char).unwrap());
    }
}
```

```rust
// src/time/mod.rs - Test data inline
#[test]
fn leap_year() {
    // not leap year - div 100 true, div 400 false
    assert!(!is_leap_year(1900u16));
    // leap year - div 400 true
    assert!(is_leap_year(2000u16));
    // not leap year - div 4 false
    assert!(!is_leap_year(2019u16));
    // leap year - div 4 true, div 100 false
    assert!(is_leap_year(2020u16));
}
```

**Location:**
- Test data defined directly in test functions; no separate fixtures directory
- Constants used for stable test data (e.g., `TEST_CHARS` array)
- Comments describe test case purpose

## Coverage

**Requirements:** No enforced coverage requirements detected

**View Coverage:**
```bash
# Using tarpaulin (if installed)
cargo tarpaulin

# Using llvm-cov (if installed)
cargo llvm-cov
```

**Current State:**
- Limited test coverage: Only URL encoding/decoding and time/leap-year logic tested
- No tests for server components (`Server`, `ThreadPool`, `Worker`, `Request`)
- No tests for main entry point
- Logger implementation not tested

## Test Types

**Unit Tests:**
- Scope: Pure functions and utility logic
- Approach: Direct assertion of function output against expected values
- Current coverage:
  - `src/url/mod.rs`: `hex_char_to_byte()`, `pct_encode()`, `pct_decode()` functions
  - `src/time/mod.rs`: `is_leap_year()` function
- Pattern: Multiple assertions per test covering different input classes (1-byte UTF-8, 2-byte, 3-byte, 4-byte characters for encoding tests)

**Integration Tests:**
- Not currently implemented
- Would typically test in `tests/` directory at crate root
- No integration test infrastructure present

**E2E Tests:**
- Not applicable; this is a backend HTTP server for a static website
- Manual testing required for HTTP response handling and file serving

## Common Patterns

**Async Testing:**
- Not applicable; no async/await in codebase
- Thread pool uses standard library `std::thread`, not async runtime

**Error Testing:**
```rust
// Testing error paths
#[test]
fn hex_byte() {
    // Tests unwrap() - function expected to succeed for valid input
    assert_eq!(value, hex_char_to_byte(char).unwrap());
}
```

**Pattern Observations:**
- Error cases not explicitly tested in current test suite
- Errors in URL parsing and time calculations not covered by tests
- `Result<T>` types tested implicitly via `.unwrap()` in success cases

## Test Execution Notes

**Coverage Gaps:**
- `src/server/mod.rs`: No tests for request handling, connection management
- `src/server/request.rs`: No tests for `Request::parse()` or `RequestMethod` conversion
- `src/server/pool.rs`: No tests for thread pool creation and job execution
- `src/server/worker.rs`: No tests for worker thread behavior
- `src/logger/mod.rs`: No tests for logger initialization or output
- `src/main.rs`: No integration tests

**Recommendations for Future Testing:**
1. Add tests for `Request::parse()` with various HTTP request formats
2. Add tests for invalid HTTP methods and versions
3. Add error case tests for URL decoding with invalid UTF-8 sequences
4. Add thread pool tests for concurrent job execution
5. Consider integration tests for full HTTP request/response cycle

---

*Testing analysis: 2026-02-28*
