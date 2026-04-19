//! Test helpers for plugin and handler unit tests.
//!
//! This module is `#[cfg(test)]`-gated at the `mod` declaration in `mod.rs`. It does NOT appear in
//! release builds.

use crate::{
    server::{
        context::Context,
        request::{Request, RequestMethod},
        response::Response,
    },
    url::Url,
};

/// Construct a `Context` for unit testing a handler or plugin.
///
/// # Panics
/// This panics if `method` is not a valid HTTP method string (e.g., "GET", "POST").
///
/// # Example
/// ```ignore
/// let mut ctx = test_context("GET", "/s/abc");
/// plugin.handle(&mut ctx).unwrap();
/// assert_eq!(ctx.response.status(), 200);
/// ```
pub fn test_context(method: &str, path: &str) -> Context {
    Context {
        request: Request {
            method: RequestMethod::try_from(method).expect("test_context: invalid HTTP method"),
            target: Url::from(path),
            host: None,
            headers: Vec::new(),
        },
        response: Response::new(200, "OK"),
        auth: None,
    }
}

/// Construct a `Context` with custom request headers for middleware tests.
///
/// Header names and values are provided as `(&str, &str)` tuples.
/// Headers are stored exactly as provided (the case is preserved).
pub fn test_context_with_headers(method: &str, path: &str, headers: &[(&str, &str)]) -> Context {
    Context {
        request: Request {
            method: RequestMethod::try_from(method).expect("test_context: invalid HTTP method"),
            target: Url::from(path),
            host: None,
            headers: headers
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        },
        response: Response::new(200, "OK"),
        auth: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_get_root_creates_valid_context() {
        let ctx = test_context("GET", "/");
        assert_eq!(
            ctx.request.method,
            RequestMethod::Get,
            "method should be GET"
        );
        assert_eq!(ctx.request.target.to_string(), "/", "target should be /");
    }

    #[test]
    fn test_context_post_path_creates_valid_context() {
        let ctx = test_context("POST", "/s/abc");
        assert_eq!(
            ctx.request.method,
            RequestMethod::Post,
            "method should be POST"
        );
    }

    #[test]
    #[should_panic(expected = "test_context: invalid HTTP method")]
    fn test_context_invalid_method_panics() {
        let _ctx = test_context("INVALID", "/");
    }

    #[test]
    fn test_context_with_headers_injects_headers() {
        let ctx = test_context_with_headers("GET", "/", &[("X-Auth", "user1")]);
        assert_eq!(ctx.request.header("x-auth"), Some("user1"));
        assert!(ctx.auth.is_none(), "auth should default to None");
    }
}
