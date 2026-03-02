//! Basic HTTP Server
//!
//! This module implements a basic HTTP server. This server leverages a thread pool to handle
//! pool to handle incoming connections. It has no third-party crate dependencies.

use std::{
    fmt,
    io::{prelude::*, BufReader},
    net::{TcpListener, TcpStream},
    result,
    sync::{mpsc, Arc, Mutex},
    thread,
};

pub use context::Context;
pub use error::{Error, Result};
pub use handler::Handler;
use pool::ThreadPool;
pub use request::Request;
#[allow(unused_imports)]
pub use request::RequestMethod;
pub use response::Response;
pub use router::Router;

use super::*;

use crate::time::DateTime;

mod context;
mod error;
mod handler;
mod pool;
mod request;
mod response;
mod router;
mod worker;

/// Tread pool size
const DEFAULT_POOL_SIZE: usize = 4;

// TODO: HTTP/1.1 Support
//  https://www.rfc-editor.org/rfc/rfc9110.txt (HTTP Semantics)
//  https://www.rfc-editor.org/rfc/rfc9111.txt (Caching)
//  https://www.rfc-editor.org/rfc/rfc9112.txt (HTTP/1.1)
//      Older: https://www.rfc-editor.org/rfc/rfc2068.txt
// TODO: URI: https://www.rfc-editor.org/rfc/rfc3986.txt
//  https://www.rfc-editor.org/rfc/rfc6454.txt (origin rules)

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
}

impl Server {
    pub fn new(addr: impl Into<String>) -> Result<Server> {
        let addr = addr.into();
        Ok(Server {
            addr: addr.clone(),
            listener: TcpListener::bind(&addr)?,
            pool: ThreadPool::build(DEFAULT_POOL_SIZE)?,
            router: Arc::new(Router::new()),
        })
    }

    /// Set the router for this server (builder pattern).
    /// If not called, all requests receive 404 Not Found.
    pub fn with_router(mut self, router: Router) -> Self {
        self.router = Arc::new(router);
        self
    }

    pub fn run(&self) -> Result<()> {
        info!("Listening for connections on {}", &self.addr);
        for stream_result in self.listener.incoming() {
            let router = Arc::clone(&self.router); // clone BEFORE move closure
            self.pool.execute(move || match stream_result {
                Ok(stream) => handle_connection(stream, router)
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

/// Send a minimal error response to the client and discard any write failure.
///
/// This is called on error paths where the BufReader has already been dropped
/// and the stream is available for writing again.
fn send_error_response(stream: &mut TcpStream, status: u16, reason: &'static str, message: &str) {
    let body = message.as_bytes().to_vec();
    let content_length = body.len().to_string();
    let mut response = Response::new(status, reason)
        .header("Content-Type", "text/plain")
        .header("Content-Length", &content_length)
        .header("Connection", "close");
    // Best-effort Date header — omit if clock fails rather than panic
    if let Ok(dt) = DateTime::now() {
        let date = dt.to_imf_fixdate();
        response = response.header("Date", &date);
    }
    let response = response.body(body);
    // Best-effort write: if write fails, there is nothing more to do (primary error captured)
    let _ = response.write_to(stream);
}

fn handle_connection(mut stream: TcpStream, router: Arc<Router>) -> Result<()> {
    info!("handling a connection");

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
                    // We cannot call send_error_response here — reader still borrows stream.
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

    let request = match Request::parse(&http_request) {
        Ok(r) => r,
        Err(e) => {
            send_error_response(&mut stream, 400, "Bad Request", &e.to_string());
            return Err(e);
        }
    };

    info!("{}", http_request[0]);
    info!("Method: {}", request.method);
    info!("Target: {}", request.target);
    debug!("Request ({:?}): {:#?}", stream.peer_addr()?, http_request);

    let mut ctx = Context {
        request,
        response: Response::new(200, "OK"),
    };

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

    // Inject Date header after dispatch (HTTP-03: every response must have Date)
    if let Ok(dt) = DateTime::now() {
        ctx.response.add_header("Date", &dt.to_imf_fixdate());
    }

    ctx.response.write_to(&mut stream)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};
    use std::thread;

    fn spawn_handle_connection_test(send_bytes: &'static [u8], router: Arc<Router>) -> Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let client_thread = thread::spawn(move || {
            let mut client = TcpStream::connect(addr).unwrap();
            client.write_all(send_bytes).unwrap();
            // Read until server closes — keeps connection alive until handler finishes
            let mut buf = Vec::new();
            let _ = client.read_to_end(&mut buf);
        });
        let (stream, _) = listener.accept().unwrap();
        let result = handle_connection(stream, router);
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
        handle_connection(stream, Arc::new(router)).unwrap();
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
        handle_connection(stream, Arc::new(Router::new())).unwrap();
        let response = client_thread.join().unwrap();
        assert!(
            response.contains("HTTP/1.1 404"),
            "expected 404, got: {:?}",
            response
        );
    }
}
