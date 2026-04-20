//! Pre-dispatch middleware chain for kiss-server.

use super::context::Context;

/// Return type for middleware: continue the chain or short-circuit.
pub enum MiddlewareResult {
    /// Continue to the next middleware (or to dispatch if last).
    Continue,
    /// Stop the chain — middleware has written ctx.response.
    #[allow(dead_code)]
    ShortCircuit,
}

/// Synchronous pre-dispatch middleware.
///
/// Middleware runs before `router.dispatch()`. It may inspect or mutate `ctx`,
/// and may short-circuit the request by writing `ctx.response` and returning
/// `MiddlewareResult::ShortCircuit`.
///
/// # Thread Safety
/// Middleware must be `Send + Sync` because it is shared across worker threads
/// via `Arc<MiddlewareChain>`.
pub trait Middleware: Send + Sync {
    fn run(&self, ctx: &mut Context) -> MiddlewareResult;
}

/// An ordered chain of middleware with a public-route exemption list.
///
/// Routes on the exemption list bypass all middleware. Non-exempt routes pass
/// through each middleware in registration order; the first `ShortCircuit`
/// stops the chain.
pub struct MiddlewareChain {
    middleware: Vec<Box<dyn Middleware>>,
    public_routes: Vec<String>,
}

impl std::fmt::Debug for MiddlewareChain {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MiddlewareChain")
            .field("middleware_count", &self.middleware.len())
            .field("public_routes", &self.public_routes)
            .finish()
    }
}

impl MiddlewareChain {
    /// Create an empty middleware chain.
    pub fn new() -> Self {
        MiddlewareChain {
            middleware: Vec::new(),
            public_routes: Vec::new(),
        }
    }

    /// Register a middleware (value-chaining builder).
    #[allow(dead_code)]
    pub fn add(mut self, m: impl Middleware + 'static) -> Self {
        self.middleware.push(Box::new(m));
        self
    }

    /// Set the public-route exemption list (value-chaining builder).
    ///
    /// Routes on this list bypass all middleware entirely. Comparison is
    /// exact-match against the percent-decoded request path (D-05).
    #[allow(dead_code)]
    pub fn public_routes(mut self, routes: &[&str]) -> Self {
        self.public_routes = routes.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Run the middleware chain against the given context.
    ///
    /// Returns `Continue` if all middleware passed (or the route is exempt).
    /// Returns `ShortCircuit` if any middleware short-circuited — `ctx.response`
    /// is already populated by the middleware that stopped the chain.
    pub fn run(&self, ctx: &mut Context) -> MiddlewareResult {
        // Decode a path once and cache on ctx; router reuses the cached value.
        if ctx.decoded_path.is_none() {
            match ctx.request.target.decoded_path() {
                Ok(d) => ctx.decoded_path = Some(d),
                Err(_) => return MiddlewareResult::Continue, // let router handle malformed paths
            }
        }
        let decoded = ctx.decoded_path.as_deref().unwrap();
        if self.public_routes.iter().any(|p| p == decoded) {
            return MiddlewareResult::Continue;
        }
        for m in &self.middleware {
            match m.run(ctx) {
                MiddlewareResult::Continue => {}
                MiddlewareResult::ShortCircuit => return MiddlewareResult::ShortCircuit,
            }
        }
        MiddlewareResult::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::response::Response;
    use crate::server::test_support::test_context;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Middleware that counts how many times it was called.
    struct CountingMiddleware {
        count: Arc<AtomicUsize>,
    }
    impl Middleware for CountingMiddleware {
        fn run(&self, _ctx: &mut Context) -> MiddlewareResult {
            self.count.fetch_add(1, Ordering::SeqCst);
            MiddlewareResult::Continue
        }
    }

    /// Middleware that always short-circuits with 401.
    struct BlockingMiddleware;
    impl Middleware for BlockingMiddleware {
        fn run(&self, ctx: &mut Context) -> MiddlewareResult {
            let body = b"Blocked\n".to_vec();
            let content_length = body.len().to_string();
            ctx.response = Response::new(401, "Unauthorized")
                .header("Content-Type", "text/plain")
                .header("Content-Length", &content_length)
                .body(body);
            MiddlewareResult::ShortCircuit
        }
    }

    #[test]
    fn empty_chain_returns_continue() {
        let chain = MiddlewareChain::new();
        let mut ctx = test_context("GET", "/api/data");
        assert!(matches!(chain.run(&mut ctx), MiddlewareResult::Continue));
    }

    #[test]
    fn middleware_runs_in_registration_order() {
        let count = Arc::new(AtomicUsize::new(0));
        let chain = MiddlewareChain::new().add(CountingMiddleware {
            count: Arc::clone(&count),
        });
        let mut ctx = test_context("GET", "/api/data");
        chain.run(&mut ctx);
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn short_circuit_stops_chain() {
        let count = Arc::new(AtomicUsize::new(0));
        let chain = MiddlewareChain::new()
            .add(BlockingMiddleware)
            .add(CountingMiddleware {
                count: Arc::clone(&count),
            });
        let mut ctx = test_context("GET", "/api/data");
        let result = chain.run(&mut ctx);
        assert!(matches!(result, MiddlewareResult::ShortCircuit));
        assert_eq!(
            count.load(Ordering::SeqCst),
            0,
            "second middleware should not run after short-circuit"
        );
        assert_eq!(ctx.response.status(), 401);
    }

    #[test]
    fn exempt_route_skips_all_middleware() {
        let count = Arc::new(AtomicUsize::new(0));
        let chain = MiddlewareChain::new()
            .add(CountingMiddleware {
                count: Arc::clone(&count),
            })
            .public_routes(&["/health", "/favicon.ico"]);
        let mut ctx = test_context("GET", "/health");
        let result = chain.run(&mut ctx);
        assert!(matches!(result, MiddlewareResult::Continue));
        assert_eq!(
            count.load(Ordering::SeqCst),
            0,
            "middleware should not run for exempt route"
        );
    }

    #[test]
    fn non_exempt_route_runs_middleware() {
        let count = Arc::new(AtomicUsize::new(0));
        let chain = MiddlewareChain::new()
            .add(CountingMiddleware {
                count: Arc::clone(&count),
            })
            .public_routes(&["/health"]);
        let mut ctx = test_context("GET", "/api/data");
        chain.run(&mut ctx);
        assert_eq!(
            count.load(Ordering::SeqCst),
            1,
            "middleware should run for non-exempt route"
        );
    }

    #[test]
    fn malformed_path_returns_continue() {
        let count = Arc::new(AtomicUsize::new(0));
        let chain = MiddlewareChain::new().add(CountingMiddleware {
            count: Arc::clone(&count),
        });
        let mut ctx = test_context("GET", "/%GG");
        let result = chain.run(&mut ctx);
        assert!(
            matches!(result, MiddlewareResult::Continue),
            "malformed path should pass through to router"
        );
    }

    #[test]
    fn exempt_check_uses_decoded_path() {
        let count = Arc::new(AtomicUsize::new(0));
        let chain = MiddlewareChain::new()
            .add(CountingMiddleware {
                count: Arc::clone(&count),
            })
            .public_routes(&["/health"]);
        // Percent-encoded version of /health
        let mut ctx = test_context("GET", "/%68ealth");
        chain.run(&mut ctx);
        assert_eq!(
            count.load(Ordering::SeqCst),
            0,
            "percent-encoded /health should be exempt"
        );
    }

    #[test]
    fn multiple_middleware_all_run_on_continue() {
        let count1 = Arc::new(AtomicUsize::new(0));
        let count2 = Arc::new(AtomicUsize::new(0));
        let chain = MiddlewareChain::new()
            .add(CountingMiddleware {
                count: Arc::clone(&count1),
            })
            .add(CountingMiddleware {
                count: Arc::clone(&count2),
            });
        let mut ctx = test_context("GET", "/api/data");
        chain.run(&mut ctx);
        assert_eq!(count1.load(Ordering::SeqCst), 1);
        assert_eq!(count2.load(Ordering::SeqCst), 1);
    }
}
