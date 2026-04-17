//! Stub authentication middleware for MVP auth strategy (AUTH-02).
//!
//! Reads `X-Authenticated-User` header (set by Lambda@Edge upstream).
//! If present and non-empty, populates `ctx.auth`. If absent or empty
//! on a non-exempt route, short-circuits with 401 Unauthorized.

use super::{
    context::{AuthClaims, Context},
    middleware::{Middleware, MiddlewareResult},
    response::Response,
};

/// Stub auth middleware that trusts the `X-Authenticated-User` header.
///
/// In production, Lambda@Edge validates the JWT and sets this header.
/// The Rust server trusts it because only CloudFront can reach the origin.
/// Direct-to-origin access bypasses auth — accepted because EC2 security group
/// restricts port 80 to CloudFront IP ranges only (Phase 17, T-22-01).
#[allow(dead_code)]
pub struct AuthMiddleware;

#[allow(dead_code)]
impl AuthMiddleware {
    pub fn new() -> Self {
        AuthMiddleware
    }
}

impl Middleware for AuthMiddleware {
    fn run(&self, ctx: &mut Context) -> MiddlewareResult {
        match ctx.request.header("x-authenticated-user") {
            Some(user_id) if !user_id.trim().is_empty() => {
                ctx.auth = Some(AuthClaims {
                    user_id: user_id.trim().to_string(),
                });
                MiddlewareResult::Continue
            }
            _ => {
                let body = b"Unauthorized\n".to_vec();
                let content_length = body.len().to_string();
                ctx.response = Response::new(401, "Unauthorized")
                    .header("Content-Type", "text/plain")
                    .header("Content-Length", &content_length)
                    .header("Connection", "close")
                    .body(body);
                MiddlewareResult::ShortCircuit
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::test_support::{test_context, test_context_with_headers};

    #[test]
    fn auth_header_present_sets_ctx_auth() {
        let mw = AuthMiddleware::new();
        let mut ctx =
            test_context_with_headers("GET", "/api/data", &[("X-Authenticated-User", "alice")]);
        let result = mw.run(&mut ctx);
        assert!(matches!(result, MiddlewareResult::Continue));
        assert!(
            ctx.auth.is_some(),
            "ctx.auth should be Some when header present"
        );
        assert_eq!(ctx.auth.as_ref().unwrap().user_id, "alice");
    }

    #[test]
    fn auth_header_absent_returns_401() {
        let mw = AuthMiddleware::new();
        let mut ctx = test_context("GET", "/api/data");
        let result = mw.run(&mut ctx);
        assert!(matches!(result, MiddlewareResult::ShortCircuit));
        assert_eq!(ctx.response.status(), 401);
    }

    #[test]
    fn auth_header_empty_returns_401() {
        let mw = AuthMiddleware::new();
        let mut ctx =
            test_context_with_headers("GET", "/api/data", &[("X-Authenticated-User", "")]);
        let result = mw.run(&mut ctx);
        assert!(matches!(result, MiddlewareResult::ShortCircuit));
        assert_eq!(ctx.response.status(), 401);
    }

    #[test]
    fn auth_header_whitespace_only_returns_401() {
        let mw = AuthMiddleware::new();
        let mut ctx =
            test_context_with_headers("GET", "/api/data", &[("X-Authenticated-User", "   ")]);
        let result = mw.run(&mut ctx);
        assert!(matches!(result, MiddlewareResult::ShortCircuit));
        assert_eq!(ctx.response.status(), 401);
    }

    #[test]
    fn auth_header_trimmed_before_storing() {
        let mw = AuthMiddleware::new();
        let mut ctx =
            test_context_with_headers("GET", "/api/data", &[("X-Authenticated-User", "  bob  ")]);
        let result = mw.run(&mut ctx);
        assert!(matches!(result, MiddlewareResult::Continue));
        assert_eq!(ctx.auth.as_ref().unwrap().user_id, "bob");
    }

    #[test]
    fn auth_header_case_insensitive_lookup() {
        let mw = AuthMiddleware::new();
        let mut ctx =
            test_context_with_headers("GET", "/api/data", &[("x-authenticated-user", "carol")]);
        let result = mw.run(&mut ctx);
        assert!(matches!(result, MiddlewareResult::Continue));
        assert_eq!(ctx.auth.as_ref().unwrap().user_id, "carol");
    }
}
