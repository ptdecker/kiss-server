//! Request router: dispatches requests to registered handlers in registration order.

use super::{
    HandlerResult, Result, context::Context, error::Error, handler::Handler,
    request::RequestMethod, response::Response,
};

/// Routes incoming requests to the first registered handler whose method and path match.
///
/// Unmatched requests are handled by the registered fallback handler when set, or by the built-in
/// `NotFoundHandler` (404) when no fallback is registered.
pub struct Router {
    routes: Vec<(RequestMethod, String, Box<dyn Handler>)>,
    prefix_routes: Vec<(String, Box<dyn Handler>)>,
    fallback: Option<Box<dyn Handler>>,
}

impl std::fmt::Debug for Router {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Router")
            .field("routes_count", &self.routes.len())
            .field("prefix_routes_count", &self.prefix_routes.len())
            .field("has_fallback", &self.fallback.is_some())
            .finish()
    }
}

impl Router {
    /// Create a new empty router. All requests will receive 404 until routes are registered.
    pub fn new() -> Self {
        Router {
            routes: Vec::new(),
            prefix_routes: Vec::new(),
            fallback: None,
        }
    }

    /// Set a fallback handler for unmatched requests (value-chaining builder).
    ///
    /// When set, the fallback handler is called for any request that does not match a registered
    /// route. If no fallback is set, unmatched requests receive a built-in 404. The safety guard
    /// (dotdot rejection, invalid %-sequences) still runs before the fallback.
    pub fn set_fallback(mut self, handler: impl Handler + 'static) -> Self {
        self.fallback = Some(Box::new(handler));
        self
    }

    /// Register a handler for the given HTTP method and exact path.
    ///
    /// Routes are checked in registration order; the first match wins. Returns `Err` if `method` is
    /// not a valid HTTP method string.
    ///
    /// Not called from production `main()` (all requests go to the StaticFileHandler fallback).
    /// Retained for tests and future use.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn add(&mut self, method: &str, path: &str, handler: impl Handler + 'static) -> Result<()> {
        self.routes.push((
            RequestMethod::try_from(method).map_err(|e| Error::InvalidRequest(e.to_string()))?,
            path.to_string(),
            Box::new(handler),
        ));
        Ok(())
    }

    /// Register a handler for all requests whose decoded path starts with `prefix`.
    ///
    /// Prefix routes are checked after exact-match routes but before the fallback. Routes are
    /// tried in registration order; the first match wins (PLUG-04). Register more-specific prefixes
    /// before less-specific ones.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn add_prefix(&mut self, prefix: impl Into<String>, handler: impl Handler + 'static) {
        let prefix = prefix.into();
        assert!(
            !prefix.is_empty(),
            "plugin prefix must be non-empty (starts_with(\"\") matches all paths)"
        );
        self.prefix_routes.push((prefix, Box::new(handler)));
    }

    /// Dispatch the request in `ctx` to the first matching handler.
    ///
    /// The path is percent-decoded before routing. Invalid %-sequences and paths containing `..`
    /// components are rejected with 404 before any handler runs.
    ///
    /// If no route matches, calls the registered fallback handler when set, or the built-in
    /// NotFoundHandler (404) when no fallback is registered. Returns `Ok(())` for all cases
    /// including rejection — never returns `Err` for path rejection (callers map `Err` to 500).
    pub fn dispatch(&self, ctx: &mut Context) -> Result<()> {
        let Ok(decoded) = ctx.request.target.decoded_path() else {
            return NotFoundHandler
                .handle(ctx)
                .map_err(|e| Error::InvalidRequest(e.to_string()));
        };
        if decoded.split('/').any(|c| c == "..") {
            return NotFoundHandler
                .handle(ctx)
                .map_err(|e| Error::InvalidRequest(e.to_string()));
        }
        for (route_method, route_path, handler) in &self.routes {
            if route_method == &ctx.request.method && route_path.as_str() == decoded.as_str() {
                return handler
                    .handle(ctx)
                    .map_err(|e| Error::InvalidRequest(e.to_string()));
            }
        }
        // Prefix routes: checked after exact matches, before fallback (PLUG-04)
        for (prefix, handler) in &self.prefix_routes {
            if decoded.starts_with(prefix.as_str()) {
                return handler
                    .handle(ctx)
                    .map_err(|e| Error::InvalidRequest(e.to_string()));
            }
        }
        match &self.fallback {
            Some(h) => h
                .handle(ctx)
                .map_err(|e| Error::InvalidRequest(e.to_string())),
            None => NotFoundHandler
                .handle(ctx)
                .map_err(|e| Error::InvalidRequest(e.to_string())),
        }
    }
}

/// Built-in fallback handler: responds 404 Not Found for all unmatched requests.
struct NotFoundHandler;

impl Handler for NotFoundHandler {
    fn handle(&self, ctx: &mut Context) -> HandlerResult<()> {
        let body = b"Not Found\n".to_vec();
        let content_length = body.len().to_string();
        ctx.response = Response::new(404, "Not Found")
            .header("Content-Type", "text/plain")
            .header("Content-Length", &content_length)
            .header("Connection", "close")
            .body(body);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::{
        HandlerResult,
        context::Context,
        request::{Request, RequestMethod},
        response::Response,
    };
    use crate::url::Url;

    // Minimal handler for tests
    struct OkHandler;
    impl Handler for OkHandler {
        fn handle(&self, ctx: &mut Context) -> HandlerResult<()> {
            ctx.response = Response::new(200, "OK")
                .header("Content-Length", "2")
                .body(b"OK".to_vec());
            Ok(())
        }
    }

    fn make_ctx(method: RequestMethod, path: &str) -> Context {
        Context {
            request: Request {
                method,
                target: Url::from(path),
                host: None,
                headers: Vec::new(),
            },
            response: Response::new(200, "OK"),
            auth: None,
        }
    }

    #[test]
    fn new_router_is_empty() {
        let router = Router::new();
        assert!(router.routes.is_empty());
    }

    #[test]
    fn add_valid_method_succeeds() {
        let mut router = Router::new();
        assert!(router.add("GET", "/", OkHandler).is_ok());
        assert_eq!(router.routes.len(), 1);
    }

    #[test]
    fn add_invalid_method_returns_err() {
        let mut router = Router::new();
        assert!(router.add("FOOBAR", "/", OkHandler).is_err());
    }

    #[test]
    fn dispatch_matching_route_calls_handler() {
        let mut router = Router::new();
        router.add("GET", "/", OkHandler).unwrap();
        let mut ctx = make_ctx(RequestMethod::Get, "/");
        router.dispatch(&mut ctx).unwrap();
        // OkHandler sets status 200 via builder; inspect response by writing to buf
        let mut buf: Vec<u8> = Vec::new();
        ctx.response.write_to(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.starts_with("HTTP/1.1 200 OK"),
            "expected 200 OK, got: {:?}",
            output
        );
    }

    #[test]
    fn dispatch_unmatched_returns_404() {
        let router = Router::new();
        let mut ctx = make_ctx(RequestMethod::Get, "/missing");
        router.dispatch(&mut ctx).unwrap();
        let mut buf: Vec<u8> = Vec::new();
        ctx.response.write_to(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.starts_with("HTTP/1.1 404"),
            "expected 404, got: {:?}",
            output
        );
    }

    #[test]
    fn dispatch_percent_encoded_path_matches_decoded_route() {
        let mut router = Router::new();
        router.add("GET", "/my file.html", OkHandler).unwrap();
        let mut ctx = make_ctx(RequestMethod::Get, "/my%20file.html");
        router.dispatch(&mut ctx).unwrap();
        let mut buf: Vec<u8> = Vec::new();
        ctx.response.write_to(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.starts_with("HTTP/1.1 200 OK"),
            "expected 200 OK for percent-encoded path, got: {:?}",
            output
        );
    }

    #[test]
    fn dispatch_dotdot_returns_404() {
        let router = Router::new();
        let mut ctx = make_ctx(RequestMethod::Get, "/../etc");
        router.dispatch(&mut ctx).unwrap();
        let mut buf: Vec<u8> = Vec::new();
        ctx.response.write_to(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.starts_with("HTTP/1.1 404"),
            "expected 404 for dotdot path, got: {:?}",
            output
        );
    }

    #[test]
    fn dispatch_encoded_dotdot_returns_404() {
        let router = Router::new();
        let mut ctx = make_ctx(RequestMethod::Get, "/%2E%2E/etc");
        router.dispatch(&mut ctx).unwrap();
        let mut buf: Vec<u8> = Vec::new();
        ctx.response.write_to(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.starts_with("HTTP/1.1 404"),
            "expected 404 for encoded dotdot path, got: {:?}",
            output
        );
    }

    #[test]
    fn dispatch_invalid_percent_returns_404() {
        let router = Router::new();
        let mut ctx = make_ctx(RequestMethod::Get, "/%GG");
        router.dispatch(&mut ctx).unwrap();
        let mut buf: Vec<u8> = Vec::new();
        ctx.response.write_to(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.starts_with("HTTP/1.1 404"),
            "expected 404 for invalid percent sequence, got: {:?}",
            output
        );
    }

    #[test]
    fn dispatch_dotdot_returns_ok_not_err() {
        let router = Router::new();
        let mut ctx = make_ctx(RequestMethod::Get, "/../etc");
        // dispatch() must return Ok(()) for path rejection, not Err — callers map Err to 500
        assert!(
            router.dispatch(&mut ctx).is_ok(),
            "dispatch should return Ok(()) for dotdot path, not Err"
        );
    }

    #[test]
    fn dispatch_first_match_wins() {
        struct StatusHandler(u16, &'static str);
        impl Handler for StatusHandler {
            fn handle(&self, ctx: &mut Context) -> HandlerResult<()> {
                ctx.response = Response::new(self.0, self.1).header("Content-Length", "0");
                Ok(())
            }
        }
        let mut router = Router::new();
        router.add("GET", "/", StatusHandler(200, "OK")).unwrap();
        router
            .add("GET", "/", StatusHandler(201, "Created"))
            .unwrap();
        let mut ctx = make_ctx(RequestMethod::Get, "/");
        router.dispatch(&mut ctx).unwrap();
        let mut buf: Vec<u8> = Vec::new();
        ctx.response.write_to(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.starts_with("HTTP/1.1 200"),
            "first match should win, got: {:?}",
            output
        );
    }

    // --- Fallback slot tests ---

    // Minimal fallback handler that writes 200
    struct OkFallbackHandler;
    impl Handler for OkFallbackHandler {
        fn handle(&self, ctx: &mut Context) -> HandlerResult<()> {
            ctx.response = Response::new(200, "OK")
                .header("Content-Length", "8")
                .body(b"fallback".to_vec());
            Ok(())
        }
    }

    #[test]
    fn new_router_has_no_fallback() {
        // Router::new() without set_fallback — fallback field is None
        let router = Router::new();
        assert!(
            router.fallback.is_none(),
            "new router should have fallback = None"
        );
    }

    #[test]
    fn set_fallback_returns_self_for_chaining() {
        // set_fallback() must return Self (value-chaining)
        // This test verifies the builder pattern compiles and produces a Router
        let router = Router::new().set_fallback(OkFallbackHandler);
        assert!(
            router.fallback.is_some(),
            "fallback should be Some after set_fallback"
        );
    }

    #[test]
    fn dispatch_with_fallback_unmatched_calls_fallback() {
        // Router with OkFallbackHandler set, unmatched GET /missing -> 200
        let router = Router::new().set_fallback(OkFallbackHandler);
        let mut ctx = make_ctx(RequestMethod::Get, "/missing");
        router.dispatch(&mut ctx).unwrap();
        let mut buf: Vec<u8> = Vec::new();
        ctx.response.write_to(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.starts_with("HTTP/1.1 200"),
            "expected fallback 200, got: {:?}",
            output
        );
    }

    #[test]
    fn dispatch_with_fallback_matched_route_not_fallback() {
        // Router with a fallback set, matched GET / -> registered route handler, NOT fallback
        let mut router = Router::new().set_fallback(OkFallbackHandler);
        router.add("GET", "/", OkHandler).unwrap();
        // OkHandler writes body "OK"; OkFallbackHandler writes body "fallback"
        // Both return 200, so check body to distinguish which was called
        let mut ctx = make_ctx(RequestMethod::Get, "/");
        router.dispatch(&mut ctx).unwrap();
        let mut buf: Vec<u8> = Vec::new();
        ctx.response.write_to(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.contains("OK") && !output.contains("fallback"),
            "registered route should win over fallback, got: {:?}",
            output
        );
    }

    #[test]
    fn dispatch_with_fallback_dotdot_returns_404() {
        // Safety guard runs before fallback — dotdot still gets 404
        let router = Router::new().set_fallback(OkFallbackHandler);
        let mut ctx = make_ctx(RequestMethod::Get, "/../etc");
        router.dispatch(&mut ctx).unwrap();
        let mut buf: Vec<u8> = Vec::new();
        ctx.response.write_to(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.starts_with("HTTP/1.1 404"),
            "dotdot path should still get 404 even with fallback set, got: {:?}",
            output
        );
    }

    #[test]
    fn dispatch_with_fallback_invalid_percent_returns_404() {
        // Safety guard runs before fallback — invalid %-sequence still gets 404
        let router = Router::new().set_fallback(OkFallbackHandler);
        let mut ctx = make_ctx(RequestMethod::Get, "/%GG");
        router.dispatch(&mut ctx).unwrap();
        let mut buf: Vec<u8> = Vec::new();
        ctx.response.write_to(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.starts_with("HTTP/1.1 404"),
            "invalid %-sequence should still get 404 even with fallback set, got: {:?}",
            output
        );
    }

    #[test]
    fn dispatch_no_fallback_unmatched_returns_404() {
        // Router with no fallback, unmatched GET /missing -> built-in NotFoundHandler (404)
        let router = Router::new();
        let mut ctx = make_ctx(RequestMethod::Get, "/missing");
        router.dispatch(&mut ctx).unwrap();
        let mut buf: Vec<u8> = Vec::new();
        ctx.response.write_to(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.starts_with("HTTP/1.1 404"),
            "no-fallback router should return 404 for unmatched, got: {:?}",
            output
        );
    }

    #[test]
    fn dispatch_unmatched_body_has_trailing_newline() {
        // NotFoundHandler body must match not_found() helper: "Not Found\n" (with newline)
        // Regression test for UAT test 6: path traversal returned 404 without trailing a newline
        let router = Router::new();
        let mut ctx = make_ctx(RequestMethod::Get, "/missing");
        router.dispatch(&mut ctx).unwrap();
        let mut buf: Vec<u8> = Vec::new();
        ctx.response.write_to(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        let sep_pos = output
            .find("\r\n\r\n")
            .expect("no blank separator in 404 response");
        let body = &output[sep_pos + 4..];
        assert_eq!(
            body, "Not Found\n",
            "NotFoundHandler body must end with newline, got: {:?}",
            body
        );
    }

    // --- Prefix route tests (Phase 21: PLUG-02, PLUG-04) ---

    #[test]
    fn add_prefix_registers_prefix_route() {
        let mut router = Router::new();
        router.add_prefix("/s", OkHandler);
        assert_eq!(
            router.prefix_routes.len(),
            1,
            "expected 1 prefix route after add_prefix"
        );
    }

    #[test]
    fn dispatch_prefix_route_matches_starts_with() {
        let mut router = Router::new();
        router.add_prefix("/s", OkHandler);
        let mut ctx = make_ctx(RequestMethod::Get, "/s/abc");
        router.dispatch(&mut ctx).unwrap();
        let mut buf: Vec<u8> = Vec::new();
        ctx.response.write_to(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.starts_with("HTTP/1.1 200"),
            "prefix /s should match /s/abc, got: {:?}",
            output
        );
    }

    #[test]
    fn dispatch_prefix_route_exact_prefix_matches() {
        let mut router = Router::new();
        router.add_prefix("/s", OkHandler);
        let mut ctx = make_ctx(RequestMethod::Get, "/s");
        router.dispatch(&mut ctx).unwrap();
        let mut buf: Vec<u8> = Vec::new();
        ctx.response.write_to(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.starts_with("HTTP/1.1 200"),
            "prefix /s should match exact /s, got: {:?}",
            output
        );
    }

    #[test]
    fn dispatch_prefix_route_no_match_returns_404() {
        let mut router = Router::new();
        router.add_prefix("/s", OkHandler);
        let mut ctx = make_ctx(RequestMethod::Get, "/other");
        router.dispatch(&mut ctx).unwrap();
        let mut buf: Vec<u8> = Vec::new();
        ctx.response.write_to(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.starts_with("HTTP/1.1 404"),
            "prefix /s should not match /other, got: {:?}",
            output
        );
    }

    #[test]
    fn dispatch_exact_route_wins_over_prefix_route() {
        struct ExactHandler;
        impl Handler for ExactHandler {
            fn handle(&self, ctx: &mut Context) -> HandlerResult<()> {
                ctx.response = Response::new(201, "Created").header("Content-Length", "0");
                Ok(())
            }
        }
        let mut router = Router::new();
        router.add("GET", "/health", ExactHandler).unwrap();
        router.add_prefix("/h", OkHandler);
        let mut ctx = make_ctx(RequestMethod::Get, "/health");
        router.dispatch(&mut ctx).unwrap();
        let mut buf: Vec<u8> = Vec::new();
        ctx.response.write_to(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.starts_with("HTTP/1.1 201"),
            "exact /health should win over prefix /h, got: {:?}",
            output
        );
    }

    #[test]
    fn dispatch_prefix_first_match_wins() {
        struct SecondHandler;
        impl Handler for SecondHandler {
            fn handle(&self, ctx: &mut Context) -> HandlerResult<()> {
                ctx.response = Response::new(201, "Created").header("Content-Length", "0");
                Ok(())
            }
        }
        let mut router = Router::new();
        router.add_prefix("/s", OkHandler);
        router.add_prefix("/s", SecondHandler);
        let mut ctx = make_ctx(RequestMethod::Get, "/s/abc");
        router.dispatch(&mut ctx).unwrap();
        let mut buf: Vec<u8> = Vec::new();
        ctx.response.write_to(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.starts_with("HTTP/1.1 200"),
            "first prefix registration should win, got: {:?}",
            output
        );
    }

    #[test]
    fn dispatch_longer_prefix_wins_when_registered_first() {
        struct LongPrefixHandler;
        impl Handler for LongPrefixHandler {
            fn handle(&self, ctx: &mut Context) -> HandlerResult<()> {
                ctx.response = Response::new(201, "Created").header("Content-Length", "0");
                Ok(())
            }
        }
        let mut router = Router::new();
        router.add_prefix("/s/featured", LongPrefixHandler);
        router.add_prefix("/s", OkHandler);
        let mut ctx = make_ctx(RequestMethod::Get, "/s/featured/item");
        router.dispatch(&mut ctx).unwrap();
        let mut buf: Vec<u8> = Vec::new();
        ctx.response.write_to(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.starts_with("HTTP/1.1 201"),
            "longer prefix /s/featured registered first should win, got: {:?}",
            output
        );
    }

    #[test]
    fn dispatch_prefix_route_dotdot_returns_404() {
        let mut router = Router::new();
        router.add_prefix("/s", OkHandler);
        let mut ctx = make_ctx(RequestMethod::Get, "/../s/abc");
        router.dispatch(&mut ctx).unwrap();
        let mut buf: Vec<u8> = Vec::new();
        ctx.response.write_to(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.starts_with("HTTP/1.1 404"),
            "dotdot should be rejected before prefix matching, got: {:?}",
            output
        );
    }

    #[test]
    fn dispatch_prefix_with_fallback_unmatched_goes_to_fallback() {
        let mut router = Router::new().set_fallback(OkFallbackHandler);
        router.add_prefix("/s", OkHandler);
        let mut ctx = make_ctx(RequestMethod::Get, "/other");
        router.dispatch(&mut ctx).unwrap();
        let mut buf: Vec<u8> = Vec::new();
        ctx.response.write_to(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.contains("fallback"),
            "unmatched prefix should fall through to fallback, got: {:?}",
            output
        );
    }
}
