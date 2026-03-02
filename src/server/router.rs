//! Request router: dispatches requests to registered handlers in registration order.

// These items are unused in the binary while handle_connection is still hard-coded.
// They are consumed in Plan 03-03 when handle_connection is wired to Router::dispatch.
#![allow(dead_code)]

use super::{
    context::Context, handler::Handler, request::RequestMethod, response::Response, Result,
};

/// Routes incoming requests to the first registered handler whose method and path match.
///
/// Unmatched requests are handled by the built-in `NotFoundHandler` fallback (404).
pub struct Router {
    routes: Vec<(RequestMethod, String, Box<dyn Handler>)>,
}

impl Router {
    /// Create a new empty router. All requests will receive 404 until routes are registered.
    pub fn new() -> Self {
        Router { routes: Vec::new() }
    }

    /// Register a handler for the given HTTP method and exact path.
    ///
    /// Routes are checked in registration order; first match wins.
    /// Returns `Err` if `method` is not a valid HTTP method string.
    pub fn add(&mut self, method: &str, path: &str, handler: impl Handler + 'static) -> Result<()> {
        let method = RequestMethod::try_from(method)?;
        self.routes
            .push((method, path.to_string(), Box::new(handler)));
        Ok(())
    }

    /// Dispatch the request in `ctx` to the first matching handler.
    ///
    /// Path comparison uses `ctx.request.target.to_string()` — Url's Display impl returns
    /// the raw path string (e.g. "/"). There is no `.path()` method on Url.
    ///
    /// If no route matches, the built-in NotFoundHandler writes a 404 response.
    pub fn dispatch(&self, ctx: &mut Context) -> Result<()> {
        let method = &ctx.request.method;
        let path = ctx.request.target.to_string();
        for (route_method, route_path, handler) in &self.routes {
            if route_method == method && route_path.as_str() == path.as_str() {
                return handler.handle(ctx);
            }
        }
        NotFoundHandler.handle(ctx)
    }
}

/// Built-in fallback handler: responds 404 Not Found for all unmatched requests.
struct NotFoundHandler;

impl Handler for NotFoundHandler {
    fn handle(&self, ctx: &mut Context) -> Result<()> {
        let body = b"Not Found".to_vec();
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
        context::Context,
        request::{Request, RequestMethod},
        response::Response,
    };
    use crate::url::Url;

    // Minimal handler for tests
    struct OkHandler;
    impl Handler for OkHandler {
        fn handle(&self, ctx: &mut Context) -> Result<()> {
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
            },
            response: Response::new(200, "OK"),
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
    fn dispatch_first_match_wins() {
        struct StatusHandler(u16, &'static str);
        impl Handler for StatusHandler {
            fn handle(&self, ctx: &mut Context) -> Result<()> {
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
}
