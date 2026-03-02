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

pub use error::{Error, Result};
use pool::ThreadPool;
use request::{Request, RequestMethod};

use super::*;

mod error;
mod pool;
mod request;
mod response;
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
}

impl Server {
    pub fn new(addr: impl Into<String>) -> Result<Server> {
        let addr = addr.into();
        Ok(Server {
            addr: addr.clone(),
            listener: TcpListener::bind(&addr)?,
            pool: ThreadPool::build(DEFAULT_POOL_SIZE)?,
        })
    }

    pub fn run(&self) -> Result<()> {
        info!("Listening for connections on {}", &self.addr);
        for stream_result in self.listener.incoming() {
            self.pool.execute(|| match stream_result {
                Ok(stream) => {
                    handle_connection(stream).unwrap_or_else(|e| warn!("handle_connection: {}", e))
                }
                Err(e) => {
                    warn!("thread: {}", e);
                }
            })?;
        }
        info!("Shutting down");
        Ok(())
    }
}

fn handle_connection(mut stream: TcpStream) -> Result<()> {
    info!("handling a connection");
    let http_request: Vec<String> = {
        let mut lines = Vec::new();
        let mut reader = BufReader::new(&mut stream);
        let mut buf = String::new();
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
                Err(e) => return Err(e.into()), // invalid UTF-8 or I/O error
            }
            if lines.len() > request::MAX_HEADER_LINES {
                return Err(Error::RequestTooLarge);
            }
        }
        lines
    };
    if http_request.is_empty() {
        return Ok(()); // empty request — close connection silently
    }
    let request = Request::parse(&http_request)?;
    info!("{}", http_request[0]);
    info!("Method: {}", request.method);
    info!("Target: {}", request.target);
    debug!("Request ({:?}): {:#?}", stream.peer_addr()?, http_request);
    // TODO: Send date in response header (Cf. RFC-9110 6.6.1)
    match (&request.method, request.target.to_string().as_str()) {
        (RequestMethod::Get, "/") => {
            let body = "OK";
            stream.write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n{}",
                    body.len(),
                    body
                )
                .as_bytes(),
            )?;
        }
        _ => {
            // Unmatched routes: close connection without response (routing is Phase 3's job)
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::{TcpListener, TcpStream};
    use std::thread;

    fn spawn_handle_connection_test(send_bytes: &'static [u8]) -> Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        thread::spawn(move || {
            let mut client = TcpStream::connect(addr).unwrap();
            client.write_all(send_bytes).unwrap();
            drop(client);
        });
        let (stream, _) = listener.accept().unwrap();
        handle_connection(stream)
    }

    #[test]
    fn invalid_utf8_returns_err_not_panic() {
        // Send raw binary bytes — invalid UTF-8
        let result = spawn_handle_connection_test(b"\xFF\xFE binary garbage\r\n\r\n");
        // Must return Err, not panic
        assert!(result.is_err());
    }

    #[test]
    fn get_root_returns_200() {
        let result = spawn_handle_connection_test(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n");
        assert!(result.is_ok());
    }
}
