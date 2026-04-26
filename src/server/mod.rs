//! Basic HTTP Server
//!
//! This module implements a basic HTTP server. This server leverages a thread pool to handle
//! pool to handle incoming connections. It has no third-party crate dependencies.

use log::{debug, info, warn};
use std::{
    fmt,
    io::{BufReader, prelude::*},
    net::{TcpListener, TcpStream},
    result,
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

pub use auth::{AuthMiddleware, VhostAuthConfig};
#[allow(unused_imports)]
pub use context::AuthClaims;
pub use context::Context;
pub use error::{Error, Result};
pub use handler::Handler;
/// Handler-compatible Result re-exported from SDK.
pub use kiss_plugin_sdk::Result as HandlerResult;
use middleware::MiddlewareResult as MwResult;
#[allow(unused_imports)]
pub use middleware::MiddlewareResult;
pub use middleware::{Middleware, MiddlewareChain};
use pool::ThreadPool;
#[allow(unused_imports)]
pub use request::Request;
pub use request::RequestMethod;
pub use response::Response;
pub use router::Router;

use crate::time::DateTime;

mod auth;
mod context;
mod error;
mod handler;
mod middleware;
mod plugin;
#[allow(unused_imports)]
pub use plugin::KissPlugin;
mod pool;
mod request;
mod response;
mod router;
mod worker;

#[cfg(test)]
mod test_support;

/// Tread pool size
const DEFAULT_POOL_SIZE: usize = 4;

/// When true, inject an X-Powered-By header into normal HTTP responses.
const ENABLE_POWERED_BY: bool = true;

/// Read timeout for inbound connections (seconds). Workers drop stalled clients after this.
const READ_TIMEOUT_SECS: u64 = 30;

/// A server, which listens for incoming connections and handles them.
#[derive(Debug)]
pub struct Server {
    /// The address to bind the server to.
    addr: String,
    /// The listener, which listens for incoming connections.
    listener: TcpListener,
    /// The thread pool, which manages our worker threads.
    pool: ThreadPool,
    /// The router, which dispatches requests to handlers.
    router: Arc<Router>,
    /// The middleware chain, which runs before dispatch.
    middleware: Arc<MiddlewareChain>,
}

impl Server {
    pub fn new(addr: impl Into<String>) -> Result<Server> {
        let addr = addr.into();
        Ok(Server {
            addr: addr.clone(),
            listener: TcpListener::bind(&addr)?,
            pool: ThreadPool::build(DEFAULT_POOL_SIZE)?,
            router: Arc::new(Router::new()),
            middleware: Arc::new(MiddlewareChain::new()),
        })
    }

    /// Set the router for this server (builder pattern). If not called, all requests receive 404
    /// Not Found.
    pub fn with_router(mut self, router: Router) -> Self {
        self.router = Arc::new(router);
        self
    }

    /// Set the middleware chain for this server (builder pattern). If not called, an empty chain is
    /// used (no middleware runs).
    pub fn with_middleware(mut self, chain: MiddlewareChain) -> Self {
        self.middleware = Arc::new(chain);
        self
    }

    pub fn run(&self) -> Result<()> {
        info!("Listening for connections on {}", &self.addr);
        for stream_result in self.listener.incoming() {
            let router = Arc::clone(&self.router);
            let middleware = Arc::clone(&self.middleware);
            self.pool.execute(move || match stream_result {
                Ok(stream) => handle_connection(stream, router, middleware)
                    .unwrap_or_else(|e| warn!("handle_connection: {}", e)),
                Err(e) => {
                    warn!("thread: {}", e);
                }
            })?;
        }
        info!("Shutting down");
        Ok(())
    }
}

/// Send a minimal error response to the client and discard any writing failure.
///
/// This is called on error paths where the BufReader has already been dropped, and the stream is
/// available for writing again.
fn send_error_response(stream: &mut TcpStream, status: u16, reason: &'static str, message: &str) {
    let body = message.as_bytes().to_vec();
    let content_length = body.len().to_string();
    let mut response = Response::new(status, reason)
        .header("Content-Type", "text/plain")
        .header("Content-Length", &content_length)
        .header("Connection", "close");
    // Best-effort Date header — omit if a clock fails rather than panic
    if let Ok(dt) = DateTime::now() {
        let date = dt.to_imf_fixdate();
        response = response.header("Date", &date);
    }
    let response = response.body(body);
    // Best-effort write: if write fails, there is nothing more to do (primary error captured)
    let _ = response.write_to(stream);
}

/// Inject Date, X-Powered-By, and Connection headers into ctx.response.
///
/// Called on both the normal dispatch path and the middleware short-circuit path
/// to ensure consistent HTTP response headers regardless of how the response was
/// generated.
fn inject_standard_headers(ctx: &mut Context) {
    if let Ok(dt) = DateTime::now() {
        ctx.response.add_header("Date", &dt.to_imf_fixdate());
    }
    if ENABLE_POWERED_BY {
        ctx.response.add_header(
            "X-Powered-By",
            concat!("kiss-server/", env!("CARGO_PKG_VERSION")),
        );
    }
    if !ctx.response.has_header("Connection") {
        ctx.response.add_header("Connection", "close");
    }
}

fn handle_connection(
    mut stream: TcpStream,
    router: Arc<Router>,
    middleware: Arc<MiddlewareChain>,
) -> Result<()> {
    let start = Instant::now();
    debug!("handling a connection");
    stream.set_read_timeout(Some(Duration::from_secs(READ_TIMEOUT_SECS)))?;

    // Collect header lines inside a block so BufReader drops before we write the response.
    // BufReader borrows &mut stream; it must be dropped before send_error_response can write.
    // We collect any I/O error as an Option so we can handle it after the block ends.
    let (http_request, too_large, io_error) = {
        let mut lines: Vec<String> = Vec::new();
        let mut reader = BufReader::new(&mut stream);
        let mut buf = String::new();
        let mut over_limit = false;
        let mut read_error: Option<std::io::Error> = None;
        loop {
            buf.clear();
            match reader.read_line(&mut buf) {
                Ok(0) => break, // EOF
                Ok(_) => {
                    let line = buf.trim_end_matches(['\r', '\n']).to_string();
                    if line.is_empty() {
                        break; // blank line = end of headers
                    }
                    lines.push(line);
                }
                Err(e) => {
                    // Collect the error and exit the loop.
                    // We cannot call send_error_response here — the reader still borrows stream.
                    // Handle after the block once BufReader is dropped.
                    read_error = Some(e);
                    break;
                }
            }
            if lines.len() > request::MAX_HEADER_LINES {
                over_limit = true;
                break;
            }
        }
        (lines, over_limit, read_error)
    };
    // BufReader is now dropped; stream is available for writing.

    if let Some(e) = io_error {
        send_error_response(&mut stream, 400, "Bad Request", &e.to_string());
        return Err(e.into());
    }

    if too_large {
        send_error_response(
            &mut stream,
            431,
            "Request Header Fields Too Large",
            &Error::RequestTooLarge.to_string(),
        );
        return Err(Error::RequestTooLarge);
    }

    if http_request.is_empty() {
        return Ok(()); // empty request — close connection silently (no response needed)
    }

    let request = match request::parse_request(&http_request) {
        Ok(r) => r,
        Err(e) => {
            send_error_response(&mut stream, 400, "Bad Request", &e.to_string());
            return Err(e);
        }
    };

    debug!("{}", http_request[0]);
    debug!("Method: {}", request.method);
    debug!("Target: {}", request.target);
    let peer = stream
        .peer_addr()
        .unwrap_or_else(|_| "0.0.0.0:0".parse().unwrap());
    debug!("Request ({:?}): {:?}", peer, http_request);

    let mut ctx = Context {
        request,
        response: Response::new(200, "OK"),
        auth: None,
        decoded_path: None,
    };

    // Run a middleware chain before dispatch (MIDL-01).
    // ShortCircuit means middleware wrote ctx.response — skip dispatch entirely (MIDL-02).
    if let MwResult::ShortCircuit = middleware.run(&mut ctx) {
        inject_standard_headers(&mut ctx);

        // Capture access log fields before write_to consumes the response
        let resp_status = ctx.response.status();
        let resp_bytes = ctx.response.body_len();
        let host_val = ctx.request.host.as_deref().unwrap_or("-").to_string();
        let method_str = ctx.request.method.to_string();
        let target_str = ctx.request.target.clone();

        ctx.response.write_to(&mut stream)?;

        let elapsed_ms = start.elapsed().as_millis();
        info!(
            target: "access",
            "{} HTTP/1.1 {} {} host={} status={} bytes={} duration_ms={}",
            peer,
            method_str,
            target_str,
            host_val,
            resp_status,
            resp_bytes,
            elapsed_ms
        );

        return Ok(());
    }

    if let Err(e) = router.dispatch(&mut ctx) {
        warn!("handler error: {}", e);
        send_error_response(
            &mut stream,
            500,
            "Internal Server Error",
            "Internal Server Error",
        );
        return Err(e);
    }

    inject_standard_headers(&mut ctx);

    // Capture access log fields before write_to consumes the response
    let resp_status = ctx.response.status();
    let resp_bytes = ctx.response.body_len();
    let host_val = ctx.request.host.as_deref().unwrap_or("-").to_string();
    let method_str = ctx.request.method.to_string();
    let target_str = ctx.request.target.clone();

    ctx.response.write_to(&mut stream)?;

    // Access log: one line per successful response (D-06, D-07, D-08)
    let elapsed_ms = start.elapsed().as_millis();
    info!(
        target: "access",
        "{} HTTP/1.1 {} {} host={} status={} bytes={} duration_ms={}",
        peer,
        method_str,
        target_str,
        host_val,
        resp_status,
        resp_bytes,
        elapsed_ms
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::thread;

    #[test]
    fn test_read_timeout_const_value() {
        assert_eq!(READ_TIMEOUT_SECS, 30, "read timeout must be 30 seconds");
    }

    fn spawn_handle_connection_test(send_bytes: &'static [u8], router: Arc<Router>) -> Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let addr = listener.local_addr()?;
        let client_thread = thread::spawn(move || {
            let mut client = TcpStream::connect(addr).unwrap();
            client.write_all(send_bytes).unwrap();
            // Read until server closes — keeps connection alive until handler finishes
            let mut buf = Vec::new();
            let _ = client.read_to_end(&mut buf);
        });
        let (stream, _) = listener.accept()?;
        let middleware = Arc::new(MiddlewareChain::new());
        let result = handle_connection(stream, router, middleware);
        client_thread.join().unwrap();
        result
    }

    #[test]
    fn invalid_utf8_returns_err_not_panic() {
        // Send raw binary bytes — invalid UTF-8
        let result = spawn_handle_connection_test(
            b"\xFF\xFE binary garbage\r\n\r\n",
            Arc::new(Router::new()),
        );
        // Must return Err, not panic
        assert!(result.is_err());
    }

    #[test]
    fn get_root_returns_200() {
        let mut router = Router::new();
        router
            .add("GET", "/", crate::handlers::RootHandler)
            .unwrap();
        let result = spawn_handle_connection_test(
            b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n",
            Arc::new(router),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn get_root_response_has_required_headers() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        // Spawn a thread that acts as the client: sends a valid request and reads the response
        let client_thread = thread::spawn(move || {
            let mut client = TcpStream::connect(addr).unwrap();
            client
                .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")
                .unwrap();
            let mut response = String::new();
            client.read_to_string(&mut response).unwrap_or(0);
            response
        });
        let (stream, _) = listener.accept().unwrap();
        let mut router = Router::new();
        router
            .add("GET", "/", crate::handlers::RootHandler)
            .unwrap();
        let middleware = Arc::new(MiddlewareChain::new());
        handle_connection(stream, Arc::new(router), middleware).unwrap();
        let response = client_thread.join().unwrap();
        assert!(response.contains("HTTP/1.1 200 OK"), "missing status line");
        assert!(response.contains("Content-Type:"), "missing Content-Type");
        assert!(
            response.contains("Content-Length:"),
            "missing Content-Length"
        );
        assert!(response.contains("Date:"), "missing Date");
        assert!(response.contains("Connection: close"), "missing Connection");
        assert!(response.contains("\r\n\r\n"), "missing blank separator");
    }

    #[test]
    fn unregistered_path_returns_404() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let client_thread = thread::spawn(move || {
            let mut client = TcpStream::connect(addr).unwrap();
            client
                .write_all(b"GET /missing HTTP/1.1\r\nHost: localhost\r\n\r\n")
                .unwrap();
            let mut response = String::new();
            client.read_to_string(&mut response).unwrap_or(0);
            response
        });
        let (stream, _) = listener.accept().unwrap();
        // Empty router — no routes registered
        let middleware = Arc::new(MiddlewareChain::new());
        handle_connection(stream, Arc::new(Router::new()), middleware).unwrap();
        let response = client_thread.join().unwrap();
        assert!(
            response.contains("HTTP/1.1 404"),
            "expected 404, got: {:?}",
            response
        );
    }

    #[test]
    fn response_contains_x_powered_by_header() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let client_thread = thread::spawn(move || {
            let mut client = TcpStream::connect(addr).unwrap();
            client
                .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")
                .unwrap();
            let mut response = String::new();
            client.read_to_string(&mut response).unwrap_or(0);
            response
        });
        let (stream, _) = listener.accept().unwrap();
        let mut router = Router::new();
        router
            .add("GET", "/", crate::handlers::RootHandler)
            .unwrap();
        let middleware = Arc::new(MiddlewareChain::new());
        handle_connection(stream, Arc::new(router), middleware).unwrap();
        let response = client_thread.join().unwrap();
        assert!(
            response.contains("X-Powered-By: kiss-server/"),
            "expected X-Powered-By header, got: {:?}",
            response
        );
    }

    /// Build an AuthMiddleware wired to "localhost" with no public paths and a
    /// test login URL. Used by integration tests that need a configured middleware.
    fn build_integration_test_middleware() -> crate::server::auth::AuthMiddleware {
        use crate::server::auth::{AuthMiddleware, VhostAuthConfig};
        use std::collections::HashMap;

        let (n_bytes, e_bytes) = crate::jwt::tests::embedded_test_modulus_exponent();

        fn der_integer(raw: &[u8]) -> Vec<u8> {
            let needs_leading_zero = !raw.is_empty() && raw[0] >= 0x80;
            let content_len = if needs_leading_zero {
                raw.len() + 1
            } else {
                raw.len()
            };
            let len_enc = crate::jwt::encode_der_length(content_len);
            let mut out = Vec::with_capacity(1 + len_enc.len() + content_len);
            out.push(0x02);
            out.extend_from_slice(&len_enc);
            if needs_leading_zero {
                out.push(0x00);
            }
            out.extend_from_slice(raw);
            out
        }

        let n_der = der_integer(&n_bytes);
        let e_der = der_integer(&e_bytes);
        let mut rsa_pubkey = vec![0x30];
        rsa_pubkey.extend_from_slice(&crate::jwt::encode_der_length(n_der.len() + e_der.len()));
        rsa_pubkey.extend_from_slice(&n_der);
        rsa_pubkey.extend_from_slice(&e_der);
        let spki = crate::jwt::wrap_rsa_pubkey_as_spki(&rsa_pubkey);

        let mut configs = HashMap::new();
        configs.insert(
            "localhost".to_string(),
            VhostAuthConfig {
                login_url: "https://test.auth0.com/login".to_string(),
                public_paths: vec![],
            },
        );
        AuthMiddleware::new(
            spki,
            configs,
            "https://test.auth0.com/".to_string(),
            "https://api.test.example.com".to_string(),
        )
    }

    /// Build a JWT token signed with the embedded test key, valid for integration tests.
    fn build_integration_test_jwt() -> String {
        let header = r#"{"alg":"RS256","typ":"JWT"}"#;
        let payload = r#"{"sub":"alice","exp":18446744073709551615,"iss":"https://test.auth0.com/","aud":"https://api.test.example.com"}"#;
        let (token, _spki, _parts) = crate::jwt::tests::build_signed_jwt(header, payload);
        token
    }

    #[test]
    fn middleware_short_circuit_returns_302_with_standard_headers() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let client_thread = thread::spawn(move || {
            let mut client = TcpStream::connect(addr).unwrap();
            // Request to /api/data WITHOUT Authorization header
            client
                .write_all(b"GET /api/data HTTP/1.1\r\nHost: localhost\r\n\r\n")
                .unwrap();
            let mut response = String::new();
            client.read_to_string(&mut response).unwrap_or(0);
            response
        });
        let (stream, _) = listener.accept().unwrap();
        let router = Arc::new(Router::new());
        let chain = MiddlewareChain::new()
            .add(build_integration_test_middleware())
            .public_routes(&["/health"]);
        let mw = Arc::new(chain);
        handle_connection(stream, router, mw).unwrap();
        let response = client_thread.join().unwrap();
        assert!(
            response.contains("HTTP/1.1 302"),
            "expected 302 redirect for unauthenticated request, got: {:?}",
            response
        );
        assert!(
            response.contains("Date:"),
            "short-circuit response must have Date header, got: {:?}",
            response
        );
        assert!(
            response.contains("Connection: close"),
            "short-circuit response must have Connection: close, got: {:?}",
            response
        );
    }

    #[test]
    fn middleware_exempt_route_bypasses_auth() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let client_thread = thread::spawn(move || {
            let mut client = TcpStream::connect(addr).unwrap();
            // Request to /health WITHOUT Authorization header
            client
                .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n")
                .unwrap();
            let mut response = String::new();
            client.read_to_string(&mut response).unwrap_or(0);
            response
        });
        let (stream, _) = listener.accept().unwrap();
        let mut router = Router::new();
        router
            .add("GET", "/health", crate::handlers::RootHandler)
            .unwrap();
        // /health is exempt via MiddlewareChain::public_routes (middleware never runs)
        let chain = MiddlewareChain::new()
            .add(build_integration_test_middleware())
            .public_routes(&["/health"]);
        let mw = Arc::new(chain);
        handle_connection(stream, Arc::new(router), mw).unwrap();
        let response = client_thread.join().unwrap();
        assert!(
            response.contains("HTTP/1.1 200"),
            "exempt /health should return 200 without auth header, got: {:?}",
            response
        );
    }

    #[test]
    fn middleware_authenticated_request_reaches_handler() {
        let token = build_integration_test_jwt();
        let auth_header = format!("Authorization: Bearer {token}");
        let request = format!("GET / HTTP/1.1\r\nHost: localhost\r\n{auth_header}\r\n\r\n");

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let client_thread = thread::spawn(move || {
            let mut client = TcpStream::connect(addr).unwrap();
            client.write_all(request.as_bytes()).unwrap();
            let mut response = String::new();
            client.read_to_string(&mut response).unwrap_or(0);
            response
        });
        let (stream, _) = listener.accept().unwrap();
        let mut router = Router::new();
        router
            .add("GET", "/", crate::handlers::RootHandler)
            .unwrap();
        let chain = MiddlewareChain::new()
            .add(build_integration_test_middleware())
            .public_routes(&["/health"]);
        let mw = Arc::new(chain);
        handle_connection(stream, Arc::new(router), mw).unwrap();
        let response = client_thread.join().unwrap();
        assert!(
            response.contains("HTTP/1.1 200"),
            "authenticated request should reach handler, got: {:?}",
            response
        );
    }

    // Checklist item 5: /s/* passes through auth middleware (not exempt)
    #[test]
    fn url_shortener_prefix_requires_auth() {
        use kiss_plugin_sdk::KissPlugin;
        use kiss_url_shortener::UrlShortener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let client_thread = thread::spawn(move || {
            let mut client = TcpStream::connect(addr).unwrap();
            // GET /s/gh WITHOUT Authorization header
            client
                .write_all(b"GET /s/gh HTTP/1.1\r\nHost: localhost\r\n\r\n")
                .unwrap();
            let mut response = String::new();
            client.read_to_string(&mut response).unwrap_or(0);
            response
        });
        let (stream, _) = listener.accept().unwrap();

        let config = kiss_plugin_sdk::PluginConfig {
            name: "url-shortener".to_string(),
            extra: Default::default(),
        };
        let p = UrlShortener::new(&config);
        let prefix = p.path_prefix().to_string();
        let mut router = Router::new();
        router.add_prefix(prefix, p).unwrap();

        let chain = MiddlewareChain::new()
            .add(build_integration_test_middleware())
            .public_routes(&["/health", "/favicon.ico"]);
        let mw = Arc::new(chain);
        handle_connection(stream, Arc::new(router), mw).unwrap();
        let response = client_thread.join().unwrap();
        assert!(
            response.contains("HTTP/1.1 302"),
            "unauthenticated /s/gh should return 302 redirect to login, got: {:?}",
            response
        );
    }

    // Checklist item 5 (corollary): authenticated /s/gh reaches plugin and returns 302
    #[test]
    fn url_shortener_authenticated_request_returns_302() {
        use kiss_plugin_sdk::KissPlugin;
        use kiss_url_shortener::UrlShortener;

        let token = build_integration_test_jwt();
        let auth_header = format!("Authorization: Bearer {token}");
        let request = format!("GET /s/gh HTTP/1.1\r\nHost: localhost\r\n{auth_header}\r\n\r\n");

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let client_thread = thread::spawn(move || {
            let mut client = TcpStream::connect(addr).unwrap();
            client.write_all(request.as_bytes()).unwrap();
            let mut response = String::new();
            client.read_to_string(&mut response).unwrap_or(0);
            response
        });
        let (stream, _) = listener.accept().unwrap();

        let config = kiss_plugin_sdk::PluginConfig {
            name: "url-shortener".to_string(),
            extra: Default::default(),
        };
        let p = UrlShortener::new(&config);
        let prefix = p.path_prefix().to_string();
        let mut router = Router::new();
        router.add_prefix(prefix, p).unwrap();

        let chain = MiddlewareChain::new()
            .add(build_integration_test_middleware())
            .public_routes(&["/health", "/favicon.ico"]);
        let mw = Arc::new(chain);
        handle_connection(stream, Arc::new(router), mw).unwrap();
        let response = client_thread.join().unwrap();
        assert!(
            response.contains("HTTP/1.1 302"),
            "authenticated /s/gh should return 302 redirect, got: {:?}",
            response
        );
        assert!(
            response.contains("Location: https://github.com/ptdecker"),
            "expected Location header, got: {:?}",
            response
        );
    }

    // Checklist item 2: /health exact match is unaffected by prefix registration
    #[test]
    fn health_route_unaffected_by_plugin_prefix() {
        use kiss_plugin_sdk::KissPlugin;
        use kiss_url_shortener::UrlShortener;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let client_thread = thread::spawn(move || {
            let mut client = TcpStream::connect(addr).unwrap();
            client
                .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\n\r\n")
                .unwrap();
            let mut response = String::new();
            client.read_to_string(&mut response).unwrap_or(0);
            response
        });
        let (stream, _) = listener.accept().unwrap();

        let config = kiss_plugin_sdk::PluginConfig {
            name: "url-shortener".to_string(),
            extra: Default::default(),
        };
        let p = UrlShortener::new(&config);
        let prefix = p.path_prefix().to_string();
        let mut router = Router::new();
        router
            .add("GET", "/health", crate::handlers::RootHandler)
            .unwrap();
        router.add_prefix(prefix, p).unwrap();

        // No auth middleware -- just verify routing
        let mw = Arc::new(MiddlewareChain::new());
        handle_connection(stream, Arc::new(router), mw).unwrap();
        let response = client_thread.join().unwrap();
        assert!(
            response.contains("HTTP/1.1 200"),
            "/health should return 200 (exact match wins over /s prefix), got: {:?}",
            response
        );
    }
}
