//! Auth middleware: validates RS256-signed JWTs from Auth0 against a per-server
//! SPKI public key, enforces per-vhost public-path exemptions, and redirects
//! unauthenticated requests to the vhost's configured login URL.
//!
//! Phase 25 wiring: see `src/main.rs::build_auth_middleware`.
//!
//! # Dead-code suppression
//!
//! Under Rust edition 2024, `dead_code` warnings fire on `pub` items that are
//! not yet reachable from `main.rs`. Plan 05 will wire `AuthMiddleware` into
//! `main.rs` at which point this attribute can be removed.
#![allow(dead_code)]

use std::collections::HashMap;

use crate::jwt;

use super::{
    context::{AuthClaims, Context},
    middleware::{Middleware, MiddlewareResult},
    response::Response,
};

/// Per-vhost auth policy. A vhost present in `AuthMiddleware::vhost_configs`
/// requires a valid JWT for any path NOT listed in `public_paths`.
/// A vhost ABSENT from the map is treated as fully public (ACFG-03).
#[derive(Debug, Clone)]
pub struct VhostAuthConfig {
    /// Full URL to redirect unauthenticated requests to (Auth0 login page).
    pub login_url: String,
    /// Exact-match list of paths that bypass the JWT check entirely.
    /// Compared against `ctx.decoded_path` (already percent-decoded by
    /// MiddlewareChain).
    pub public_paths: Vec<String>,
}

/// JWT-validating auth middleware.
///
/// The `spki_der` field carries the RSA public key fetched from Auth0's JWKS
/// endpoint at startup (see `src/jwks/mod.rs`). It is hot-path read-only;
/// the entire middleware is `Send + Sync` because all fields are immutable
/// after construction.
pub struct AuthMiddleware {
    spki_der: Vec<u8>,
    vhost_configs: HashMap<String, VhostAuthConfig>,
    issuer: String,
    audience: String,
}

impl AuthMiddleware {
    pub fn new(
        spki_der: Vec<u8>,
        vhost_configs: HashMap<String, VhostAuthConfig>,
        issuer: String,
        audience: String,
    ) -> Self {
        AuthMiddleware {
            spki_der,
            vhost_configs,
            issuer,
            audience,
        }
    }

    /// Construct a 302 redirect response to the given login URL and signal
    /// the chain to short-circuit. No body; `Content-Length: 0` is required
    /// to avoid HTTP/1.1 chunked-transfer ambiguity.
    fn redirect(&self, ctx: &mut Context, login_url: &str) -> MiddlewareResult {
        ctx.response = Response::new(302, "Found")
            .header("Location", login_url)
            .header("Content-Length", "0")
            .header("Connection", "close");
        MiddlewareResult::ShortCircuit
    }
}

impl Middleware for AuthMiddleware {
    fn run(&self, ctx: &mut Context) -> MiddlewareResult {
        // 1. Resolve per-vhost auth policy. ACFG-03: missing config = fully public.
        let host = ctx.request.host.as_deref().unwrap_or("");
        let auth_config = match self.vhost_configs.get(host) {
            None => return MiddlewareResult::Continue,
            Some(cfg) => cfg,
        };

        // 2. AMID-03: check public_paths BEFORE doing any token work.
        //    decoded_path is set by MiddlewareChain::run; if absent, fall through
        //    to Continue so the router sees the malformed path.
        let decoded = match ctx.decoded_path.as_deref() {
            Some(p) => p,
            None => return MiddlewareResult::Continue,
        };
        if auth_config.public_paths.iter().any(|p| p == decoded) {
            return MiddlewareResult::Continue;
        }

        // 3. AMID-01: extract Bearer token from Authorization header.
        let token = match ctx.request.header("authorization") {
            Some(value) => match value.strip_prefix("Bearer ") {
                Some(t) if !t.is_empty() => t,
                _ => return self.redirect(ctx, &auth_config.login_url),
            },
            None => return self.redirect(ctx, &auth_config.login_url),
        };

        // 4. parse -> verify -> extract pipeline.
        //    Any failure routes through the redirect helper. We do not distinguish
        //    failure modes in the response (no oracle).
        let parts = match jwt::parse(token) {
            Ok(p) => p,
            Err(_) => return self.redirect(ctx, &auth_config.login_url),
        };
        if jwt::verify(&parts, &self.spki_der).is_err() {
            return self.redirect(ctx, &auth_config.login_url);
        }
        let claims = match jwt::extract(&parts.payload_b64) {
            Ok(c) => c,
            Err(_) => return self.redirect(ctx, &auth_config.login_url),
        };

        // 5. Security: validate iss and aud against configured values.
        //    Without these checks, a token from a DIFFERENT Auth0 tenant or a
        //    DIFFERENT application within the same tenant would be accepted.
        if claims.iss != self.issuer {
            return self.redirect(ctx, &auth_config.login_url);
        }
        if claims.aud != self.audience {
            return self.redirect(ctx, &auth_config.login_url);
        }

        // 6. AMID-04: populate ctx.auth from sub claim.
        ctx.auth = Some(AuthClaims {
            user_id: claims.sub,
        });
        MiddlewareResult::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::test_support::{test_context, test_context_with_headers};

    // --- Test helpers ---

    const TEST_ISSUER: &str = "https://test-tenant.auth0.com/";
    const TEST_AUDIENCE: &str = "https://api.example.com";
    const TEST_HOST: &str = "example.com";

    /// Build an AuthMiddleware with one vhost ("example.com") whose public_paths
    /// are ["/public"] and login_url is "https://test-tenant.auth0.com/login".
    /// SPKI is the embedded test key's public component.
    fn build_test_middleware() -> AuthMiddleware {
        let (n_bytes, e_bytes) = crate::jwt::tests::embedded_test_modulus_exponent();
        // Reuse jwks DER assembly to mirror the production pipeline
        let n_der = der_integer(&n_bytes);
        let e_der = der_integer(&e_bytes);
        let mut rsa_pubkey = vec![0x30];
        rsa_pubkey.extend_from_slice(&crate::jwt::encode_der_length(n_der.len() + e_der.len()));
        rsa_pubkey.extend_from_slice(&n_der);
        rsa_pubkey.extend_from_slice(&e_der);
        let spki = crate::jwt::wrap_rsa_pubkey_as_spki(&rsa_pubkey);

        let mut configs = HashMap::new();
        configs.insert(
            TEST_HOST.to_string(),
            VhostAuthConfig {
                login_url: "https://test-tenant.auth0.com/login".to_string(),
                public_paths: vec!["/public".to_string()],
            },
        );
        AuthMiddleware::new(
            spki,
            configs,
            TEST_ISSUER.to_string(),
            TEST_AUDIENCE.to_string(),
        )
    }

    // Local copy of der_integer (only for test helpers; production code lives in src/jwks/mod.rs)
    fn der_integer(raw: &[u8]) -> Vec<u8> {
        let needs_leading_zero = !raw.is_empty() && raw[0] >= 0x80;
        let content_len = if needs_leading_zero {
            raw.len() + 1
        } else {
            raw.len()
        };
        let len_encoding = crate::jwt::encode_der_length(content_len);
        let mut out = Vec::with_capacity(1 + len_encoding.len() + content_len);
        out.push(0x02);
        out.extend_from_slice(&len_encoding);
        if needs_leading_zero {
            out.push(0x00);
        }
        out.extend_from_slice(raw);
        out
    }

    /// Build a Context with `host` set to TEST_HOST and `decoded_path` populated.
    fn ctx_for(method: &str, path: &str) -> Context {
        let mut ctx = test_context(method, path);
        ctx.request.host = Some(TEST_HOST.to_string());
        ctx.decoded_path = Some(path.to_string());
        ctx
    }

    fn ctx_with_auth_header(method: &str, path: &str, header_value: &str) -> Context {
        let mut ctx = test_context_with_headers(method, path, &[("Authorization", header_value)]);
        ctx.request.host = Some(TEST_HOST.to_string());
        ctx.decoded_path = Some(path.to_string());
        ctx
    }

    /// Build a JWT signed by the embedded test key with the given iss, aud, sub, exp.
    fn signed_jwt(iss: &str, aud: &str, sub: &str, exp: u64) -> String {
        let header = r#"{"alg":"RS256","typ":"JWT"}"#;
        let payload = format!(r#"{{"iss":"{iss}","aud":"{aud}","sub":"{sub}","exp":{exp}}}"#);
        let (token, _sig, _parts) = crate::jwt::tests::build_signed_jwt(header, &payload);
        token
    }

    // --- Tests ---

    #[test]
    fn auth_no_vhost_config_is_public() {
        // ACFG-03: AuthMiddleware with empty vhost_configs map -> all hosts public
        let mw = AuthMiddleware::new(
            vec![],
            HashMap::new(),
            TEST_ISSUER.to_string(),
            TEST_AUDIENCE.to_string(),
        );
        let mut ctx = ctx_for("GET", "/anything");
        assert!(matches!(mw.run(&mut ctx), MiddlewareResult::Continue));
        assert!(ctx.auth.is_none());
    }

    #[test]
    fn auth_unknown_host_is_public() {
        let mw = build_test_middleware();
        let mut ctx = test_context("GET", "/protected");
        ctx.request.host = Some("not-in-config.example.org".to_string());
        ctx.decoded_path = Some("/protected".to_string());
        assert!(matches!(mw.run(&mut ctx), MiddlewareResult::Continue));
    }

    #[test]
    fn auth_public_path_bypasses_jwt_check() {
        // AMID-03
        let mw = build_test_middleware();
        let mut ctx = ctx_for("GET", "/public");
        assert!(matches!(mw.run(&mut ctx), MiddlewareResult::Continue));
        assert!(ctx.auth.is_none(), "public path must not populate auth");
    }

    #[test]
    fn auth_public_path_does_not_require_any_header() {
        let mw = build_test_middleware();
        let mut ctx = ctx_for("GET", "/public");
        assert!(matches!(mw.run(&mut ctx), MiddlewareResult::Continue));
        assert_eq!(ctx.response.status(), 200, "response untouched on Continue");
    }

    #[test]
    fn auth_protected_path_no_authorization_header_redirects_302() {
        // AMID-02
        let mw = build_test_middleware();
        let mut ctx = ctx_for("GET", "/protected");
        assert!(matches!(mw.run(&mut ctx), MiddlewareResult::ShortCircuit));
        assert_eq!(ctx.response.status(), 302);
        assert!(
            ctx.response.has_header("Location"),
            "302 must include Location header"
        );
    }

    #[test]
    fn auth_protected_path_non_bearer_scheme_redirects() {
        let mw = build_test_middleware();
        let mut ctx = ctx_with_auth_header("GET", "/protected", "Basic dXNlcjpwYXNz");
        assert!(matches!(mw.run(&mut ctx), MiddlewareResult::ShortCircuit));
        assert_eq!(ctx.response.status(), 302);
    }

    #[test]
    fn auth_protected_path_malformed_jwt_redirects() {
        let mw = build_test_middleware();
        let mut ctx = ctx_with_auth_header("GET", "/protected", "Bearer not-a-jwt");
        assert!(matches!(mw.run(&mut ctx), MiddlewareResult::ShortCircuit));
        assert_eq!(ctx.response.status(), 302);
    }

    #[test]
    fn auth_protected_path_tampered_signature_redirects() {
        let mw = build_test_middleware();
        let mut token = signed_jwt(TEST_ISSUER, TEST_AUDIENCE, "user-1", u64::MAX);
        // Flip a byte in the signature segment (after the second '.')
        let last_dot = token.rfind('.').unwrap();
        let tampered_byte = if token.as_bytes()[last_dot + 1] != b'A' {
            'A'
        } else {
            'B'
        };
        // Replace just the first signature byte in-place
        token.replace_range(last_dot + 1..last_dot + 2, &tampered_byte.to_string());
        let header = format!("Bearer {token}");
        let mut ctx = ctx_with_auth_header("GET", "/protected", &header);
        assert!(matches!(mw.run(&mut ctx), MiddlewareResult::ShortCircuit));
        assert_eq!(ctx.response.status(), 302);
    }

    #[test]
    fn auth_protected_path_expired_token_redirects() {
        let mw = build_test_middleware();
        // exp=1 (unix epoch + 1 second) is firmly in the past
        let token = signed_jwt(TEST_ISSUER, TEST_AUDIENCE, "user-1", 1);
        let header = format!("Bearer {token}");
        let mut ctx = ctx_with_auth_header("GET", "/protected", &header);
        assert!(matches!(mw.run(&mut ctx), MiddlewareResult::ShortCircuit));
        assert_eq!(ctx.response.status(), 302);
    }

    #[test]
    fn auth_protected_path_wrong_issuer_redirects() {
        let mw = build_test_middleware();
        let token = signed_jwt(
            "https://wrong-tenant.auth0.com/",
            TEST_AUDIENCE,
            "user-1",
            u64::MAX,
        );
        let header = format!("Bearer {token}");
        let mut ctx = ctx_with_auth_header("GET", "/protected", &header);
        assert!(matches!(mw.run(&mut ctx), MiddlewareResult::ShortCircuit));
        assert_eq!(ctx.response.status(), 302);
        assert!(
            ctx.auth.is_none(),
            "ctx.auth must NOT be populated on iss mismatch"
        );
    }

    #[test]
    fn auth_protected_path_wrong_audience_redirects() {
        let mw = build_test_middleware();
        let token = signed_jwt(
            TEST_ISSUER,
            "https://wrong-audience.example.com",
            "user-1",
            u64::MAX,
        );
        let header = format!("Bearer {token}");
        let mut ctx = ctx_with_auth_header("GET", "/protected", &header);
        assert!(matches!(mw.run(&mut ctx), MiddlewareResult::ShortCircuit));
        assert_eq!(ctx.response.status(), 302);
        assert!(ctx.auth.is_none());
    }

    #[test]
    fn auth_protected_path_valid_jwt_continues_and_sets_ctx_auth() {
        // AMID-04
        let mw = build_test_middleware();
        let token = signed_jwt(TEST_ISSUER, TEST_AUDIENCE, "alice@example.com", u64::MAX);
        let header = format!("Bearer {token}");
        let mut ctx = ctx_with_auth_header("GET", "/protected", &header);
        assert!(matches!(mw.run(&mut ctx), MiddlewareResult::Continue));
        assert!(ctx.auth.is_some(), "valid JWT must populate ctx.auth");
        assert_eq!(
            ctx.auth.as_ref().unwrap().user_id,
            "alice@example.com",
            "user_id must equal sub claim"
        );
    }

    #[test]
    fn auth_decoded_path_none_continues_to_router() {
        let mw = build_test_middleware();
        let mut ctx = test_context("GET", "/protected");
        ctx.request.host = Some(TEST_HOST.to_string());
        ctx.decoded_path = None;
        assert!(
            matches!(mw.run(&mut ctx), MiddlewareResult::Continue),
            "missing decoded_path should fall through to router"
        );
    }

    #[test]
    fn auth_redirect_response_has_location_content_length_connection_headers() {
        let mw = build_test_middleware();
        let mut ctx = ctx_for("GET", "/protected");
        mw.run(&mut ctx);
        assert_eq!(ctx.response.status(), 302);
        assert!(ctx.response.has_header("Location"));
        assert!(ctx.response.has_header("Content-Length"));
        assert!(ctx.response.has_header("Connection"));
    }

    #[test]
    fn auth_bearer_with_no_token_redirects() {
        let mw = build_test_middleware();
        let mut ctx = ctx_with_auth_header("GET", "/protected", "Bearer ");
        assert!(matches!(mw.run(&mut ctx), MiddlewareResult::ShortCircuit));
        assert_eq!(ctx.response.status(), 302);
    }
}
