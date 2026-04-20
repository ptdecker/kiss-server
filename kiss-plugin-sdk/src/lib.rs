//! Plugin SDK for kiss-server: shared types and traits for plugin development.

use std::fmt;

/// Result type for plugin handlers and SDK operations.
///
/// Uses a boxed dynamic error so plugins need not depend on server-internal error types.
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

// ---------------------------------------------------------------------------
// Url
// ---------------------------------------------------------------------------

/// A basic URL type with percent-decoding support.
#[derive(Default, Debug, Clone)]
pub struct Url {
    raw_path: String,
}

impl fmt::Display for Url {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", &self.raw_path)
    }
}

impl From<&str> for Url {
    fn from(value: &str) -> Self {
        Url {
            raw_path: String::from(value),
        }
    }
}

impl Url {
    /// Returns the path component of the URL, stripping any query string.
    ///
    /// If the raw path is "/file.html?v=1", returns "/file.html".
    /// If no '?' is present, returns the full raw path.
    pub fn path(&self) -> &str {
        match self.raw_path.find('?') {
            Some(idx) => &self.raw_path[..idx],
            None => &self.raw_path,
        }
    }

    /// Percent-decodes the path component of the URL.
    ///
    /// Strips the query string first, then decodes each `%HH` sequence into its raw byte,
    /// converts the resulting byte buffer to a UTF-8 string.
    ///
    /// Returns `Err` if a `%` sequence is truncated or contains invalid hex digits.
    pub fn decoded_path(&self) -> Result<String> {
        let raw = self.path();
        let mut buf: Vec<u8> = Vec::with_capacity(raw.len());
        let mut chars = raw.chars();
        while let Some(c) = chars.next() {
            if c == '%' {
                let hi =
                    chars
                        .next()
                        .ok_or_else(|| -> Box<dyn std::error::Error + Send + Sync> {
                            "invalid %-sequence: truncated".into()
                        })?;
                let lo =
                    chars
                        .next()
                        .ok_or_else(|| -> Box<dyn std::error::Error + Send + Sync> {
                            "invalid %-sequence: truncated".into()
                        })?;
                let byte = (hex_char_to_byte(hi)? << 4) | hex_char_to_byte(lo)?;
                buf.push(byte);
            } else {
                let mut tmp = [0u8; 4];
                buf.extend_from_slice(c.encode_utf8(&mut tmp).as_bytes());
            }
        }
        String::from_utf8(buf).map_err(|e| e.to_string().into())
    }
}

/// Helper: convert a hex character to its numeric byte value.
fn hex_char_to_byte(c: char) -> Result<u8> {
    match c {
        '0'..='9' => Ok(c as u8 - b'0'),
        'a'..='f' => Ok(c as u8 - b'a' + 10),
        'A'..='F' => Ok(c as u8 - b'A' + 10),
        _ => Err(format!("The character '{}' is not a valid hexadecimal digit.", c).into()),
    }
}

// ---------------------------------------------------------------------------
// RequestMethod
// ---------------------------------------------------------------------------

/// Request Methods (RFC-9110 7.1)
///
/// Cf. <https://datatracker.ietf.org/doc/html/rfc9110#name-overview>
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum RequestMethod {
    Get,
    Head,
    Post,
    Put,
    Delete,
    Connect,
    Options,
    Trace,
}

impl fmt::Display for RequestMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let method_str = match self {
            RequestMethod::Get => "GET",
            RequestMethod::Head => "HEAD",
            RequestMethod::Post => "POST",
            RequestMethod::Put => "PUT",
            RequestMethod::Delete => "DELETE",
            RequestMethod::Connect => "CONNECT",
            RequestMethod::Options => "OPTIONS",
            RequestMethod::Trace => "TRACE",
        };
        write!(f, "{}", method_str)
    }
}

impl TryFrom<&str> for RequestMethod {
    type Error = Box<dyn std::error::Error + Send + Sync>;
    fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
        match value {
            "GET" => Ok(RequestMethod::Get),
            "HEAD" => Ok(RequestMethod::Head),
            "POST" => Ok(RequestMethod::Post),
            "PUT" => Ok(RequestMethod::Put),
            "DELETE" => Ok(RequestMethod::Delete),
            "CONNECT" => Ok(RequestMethod::Connect),
            "OPTIONS" => Ok(RequestMethod::Options),
            "TRACE" => Ok(RequestMethod::Trace),
            _ => Err(format!("invalid method: {value}").into()),
        }
    }
}

// ---------------------------------------------------------------------------
// Request
// ---------------------------------------------------------------------------

/// An HTTP/1.1 request.
#[derive(Debug, Clone)]
pub struct Request {
    /// The request method (RFC-9110 9).
    pub method: RequestMethod,
    /// The request target (RFC-9110 7.1).
    pub target: Url,
    /// The Host header value, extracted raw (not normalized). None if absent.
    pub host: Option<String>,
    /// All header lines from the request, collected as (name, value) pairs.
    pub headers: Vec<(String, String)>,
}

impl Request {
    /// Case-insensitive header lookup. Returns the value of the first matching header.
    #[allow(dead_code)]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

// ---------------------------------------------------------------------------
// Response
// ---------------------------------------------------------------------------

use std::io::Write;

/// An HTTP/1.1 response, constructed via a value-chaining builder.
///
/// Build order: Response::new(status, reason).header(k, v)...body(bytes)
/// Send order: response.write_to(&mut stream)?
pub struct Response {
    status: u16,
    reason: &'static str,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Response {
    /// Construct a new response with the given status code and reason phrase.
    pub fn new(status: u16, reason: &'static str) -> Self {
        Response {
            status,
            reason,
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    /// Add a header to the response (value-chaining builder).
    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_string(), value.to_string()));
        self
    }

    /// Set the body of the response (value-chaining builder).
    pub fn body(mut self, body: Vec<u8>) -> Self {
        self.body = body;
        self
    }

    /// Returns the HTTP status code.
    pub fn status(&self) -> u16 {
        self.status
    }

    /// Returns the body length in bytes.
    pub fn body_len(&self) -> usize {
        self.body.len()
    }

    /// Add a header to the response in place (mutating).
    ///
    /// Use this after dispatch when the response has already been built by a handler
    /// and a cross-cutting header (e.g., Date) must be injected before writing.
    pub fn add_header(&mut self, name: &str, value: &str) {
        self.headers.push((name.to_string(), value.to_string()));
    }

    /// Returns true if a header with the given name is already present (case-insensitive).
    pub fn has_header(&self, name: &str) -> bool {
        let name_lower = name.to_ascii_lowercase();
        self.headers
            .iter()
            .any(|(k, _)| k.to_ascii_lowercase() == name_lower)
    }

    /// Serialize and write the response to the given writer.
    ///
    /// Format: "HTTP/1.1 {status} {reason}\r\n{headers}\r\n{body}"
    /// Each header line ends with CRLF. The header section ends with a blank CRLF line.
    pub fn write_to(self, writer: &mut impl Write) -> std::io::Result<()> {
        // Status line
        write!(writer, "HTTP/1.1 {} {}\r\n", self.status, self.reason)?;
        // Headers
        for (name, value) in &self.headers {
            write!(writer, "{}: {}\r\n", name, value)?;
        }
        // Blank separator line
        writer.write_all(b"\r\n")?;
        // Body
        if !self.body.is_empty() {
            writer.write_all(&self.body)?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// AuthClaims
// ---------------------------------------------------------------------------

/// Authentication identity extracted by middleware.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct AuthClaims {
    pub user_id: String,
}

// ---------------------------------------------------------------------------
// Context
// ---------------------------------------------------------------------------

/// Shared mutable state for a single HTTP request/response cycle.
pub struct Context {
    pub request: Request,
    pub response: Response,
    #[allow(dead_code)]
    pub auth: Option<AuthClaims>,
    /// Cached percent-decoded path. Populated on first decode; reused by subsequent layers.
    pub decoded_path: Option<String>,
}

// ---------------------------------------------------------------------------
// PluginConfig
// ---------------------------------------------------------------------------

/// Plugin configuration container.
#[derive(Debug, Clone)]
pub struct PluginConfig {
    pub name: String,
    pub extra: std::collections::HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// Handler trait
// ---------------------------------------------------------------------------

/// A synchronous HTTP request handler.
///
/// Handlers must be Send + Sync because they are shared across worker threads.
pub trait Handler: Send + Sync {
    fn handle(&self, ctx: &mut Context) -> Result<()>;
}

// ---------------------------------------------------------------------------
// KissPlugin trait
// ---------------------------------------------------------------------------

/// Metadata extension for prefix-routed plugins.
#[allow(dead_code)]
pub trait KissPlugin: Handler {
    fn name(&self) -> &str;
    fn path_prefix(&self) -> &str;
}

// ---------------------------------------------------------------------------
// test_support module
// ---------------------------------------------------------------------------

/// Test helpers for plugin unit tests. Available to all dependents.
pub mod test_support {
    use super::*;

    /// Construct a Context for unit testing a handler or plugin.
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
            decoded_path: None,
        }
    }

    /// Construct a Context with custom request headers.
    pub fn test_context_with_headers(
        method: &str,
        path: &str,
        headers: &[(&str, &str)],
    ) -> Context {
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
            decoded_path: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::{Arc, RwLock};

    #[test]
    fn stateful_plugin_is_send_and_sync() {
        struct StatefulPlugin {
            store: Arc<RwLock<HashMap<String, String>>>,
        }

        impl Handler for StatefulPlugin {
            fn handle(&self, ctx: &mut Context) -> Result<()> {
                let _guard = self.store.read().unwrap();
                ctx.response = Response::new(200, "OK")
                    .header("Content-Length", "2")
                    .body(b"OK".to_vec());
                Ok(())
            }
        }

        impl KissPlugin for StatefulPlugin {
            fn name(&self) -> &str {
                "stateful-test"
            }
            fn path_prefix(&self) -> &str {
                "/test"
            }
        }

        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<StatefulPlugin>();
    }

    #[test]
    fn test_context_creates_valid_context() {
        let ctx = test_support::test_context("GET", "/");
        assert_eq!(ctx.request.method, RequestMethod::Get);
    }

    #[test]
    fn response_status_accessor() {
        assert_eq!(Response::new(404, "Not Found").status(), 404);
    }
}
