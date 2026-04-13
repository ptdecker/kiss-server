//! Virtual host dispatcher handler.
//!
//! Routes HTTP requests to per-domain [`super::StaticFileHandler`] instances based on
//! the normalized Host header value. Unknown domains receive a parked-domain HTML page
//! (or are forwarded to a configured default handler).

use std::collections::HashMap;

use crate::server::{Context, Handler, Response, Result};

use super::StaticFileHandler;

/// Normalize a raw Host header value for vhost dispatch (D-08).
///
/// Applies transformations in order:
/// 1. Lowercase the entire value.
/// 2. Strip trailing port (`:NNN` where NNN is all digits).
/// 3. Strip `www.` prefix.
///
/// Returns an empty string for an empty input.
pub fn normalize_host(raw: &str) -> String {
    let mut s = raw.to_ascii_lowercase();

    // Strip trailing port: find last ':' and check if everything after is digits.
    if let Some(pos) = s.rfind(':') {
        if s[pos + 1..].chars().all(|c| c.is_ascii_digit()) {
            s.truncate(pos);
        }
    }

    // Strip www. prefix.
    if let Some(rest) = s.strip_prefix("www.") {
        s = rest.to_string();
    }

    s
}

/// Dispatches requests to per-domain [`StaticFileHandler`] instances.
///
/// On each request:
/// - Extracts the raw Host header value (empty string if absent).
/// - Normalizes it via [`normalize_host`].
/// - Looks up the normalized host in the configured vhosts map.
/// - If found, delegates to that handler.
/// - If not found and `default_handler` is set, delegates to the default handler.
/// - Otherwise, returns a 200 parked-domain HTML page.
pub struct VhostDispatcher {
    vhosts: HashMap<String, StaticFileHandler>,
    default_handler: Option<StaticFileHandler>,
}

impl VhostDispatcher {
    /// Create a new VhostDispatcher.
    ///
    /// `vhosts` maps normalized domain names to their `StaticFileHandler`.
    /// `default_handler` is used for requests whose Host does not match any vhost.
    pub fn new(
        vhosts: HashMap<String, StaticFileHandler>,
        default_handler: Option<StaticFileHandler>,
    ) -> Self {
        VhostDispatcher {
            vhosts,
            default_handler,
        }
    }
}

impl Handler for VhostDispatcher {
    fn handle(&self, ctx: &mut Context) -> Result<()> {
        let raw_host = ctx.request.host.as_deref().unwrap_or("");
        let host = normalize_host(raw_host);

        if let Some(handler) = self.vhosts.get(&host) {
            return handler.handle(ctx);
        }

        if let Some(handler) = &self.default_handler {
            return handler.handle(ctx);
        }

        parked_page(ctx, &host)
    }
}

/// Build a 200 parked-domain HTML response.
fn parked_page(ctx: &mut Context, host: &str) -> Result<()> {
    let html = format!(
        "<!DOCTYPE html>\n<html>\n<head><title>Parked Domain</title></head>\n<body>\n<p>{} is parked here but has no content.</p>\n</body>\n</html>",
        host
    );
    let body = html.into_bytes();
    let content_length = body.len().to_string();
    ctx.response = Response::new(200, "OK")
        .header("Content-Type", "text/html")
        .header("Content-Length", &content_length)
        .header("Connection", "close")
        .body(body);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::{Context, Request, RequestMethod, Response};
    use crate::url::Url;
    use std::collections::HashMap;

    fn make_ctx_with_host(host: Option<&str>) -> Context {
        Context {
            request: Request {
                method: RequestMethod::Get,
                target: Url::from("/"),
                host: host.map(|h| h.to_string()),
            },
            response: Response::new(200, "OK"),
        }
    }

    fn make_ctx_with_host_and_path(host: Option<&str>, path: &str) -> Context {
        Context {
            request: Request {
                method: RequestMethod::Get,
                target: Url::from(path),
                host: host.map(|h| h.to_string()),
            },
            response: Response::new(200, "OK"),
        }
    }

    fn take_response_string(ctx: &mut Context) -> String {
        let response = std::mem::replace(&mut ctx.response, Response::new(200, "OK"));
        let mut buf: Vec<u8> = Vec::new();
        response.write_to(&mut buf).unwrap();
        String::from_utf8(buf).unwrap()
    }

    // ========== normalize_host tests ==========

    #[test]
    fn normalize_uppercase() {
        assert_eq!(normalize_host("PTODD.ORG"), "ptodd.org");
    }

    #[test]
    fn normalize_strips_port() {
        assert_eq!(normalize_host("ptodd.org:80"), "ptodd.org");
    }

    #[test]
    fn normalize_strips_www_prefix() {
        assert_eq!(normalize_host("www.ptodd.org"), "ptodd.org");
    }

    #[test]
    fn normalize_uppercase_www_and_port() {
        assert_eq!(normalize_host("WWW.PTODD.ORG:443"), "ptodd.org");
    }

    #[test]
    fn normalize_strips_non_standard_port() {
        assert_eq!(normalize_host("ptodd.org:8080"), "ptodd.org");
    }

    #[test]
    fn normalize_empty_string() {
        assert_eq!(normalize_host(""), "");
    }

    #[test]
    fn normalize_www_dot_only() {
        // "www." with nothing after strip_prefix leaves ""
        assert_eq!(normalize_host("www."), "");
    }

    // ========== VhostDispatcher tests ==========

    fn make_temp_root(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("kiss_vhost_test_{}", name));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn known_domain_dispatches_to_handler() {
        let dir = make_temp_root("known_domain");
        std::fs::write(dir.join("index.html"), b"hello from ptodd").unwrap();
        let handler = StaticFileHandler::new(dir).unwrap();
        let mut vhosts = HashMap::new();
        vhosts.insert("ptodd.org".to_string(), handler);
        let dispatcher = VhostDispatcher::new(vhosts, None);

        let mut ctx = make_ctx_with_host_and_path(Some("ptodd.org"), "/index.html");
        dispatcher.handle(&mut ctx).unwrap();
        let output = take_response_string(&mut ctx);
        assert!(
            output.starts_with("HTTP/1.1 200 OK"),
            "expected 200, got: {:?}",
            output
        );
        let sep_pos = output.find("\r\n\r\n").expect("no blank separator");
        let body = &output[sep_pos + 4..];
        assert!(
            body.contains("hello from ptodd"),
            "expected file body, got: {:?}",
            body
        );
    }

    #[test]
    fn unknown_domain_no_default_returns_parked_page() {
        let dispatcher = VhostDispatcher::new(HashMap::new(), None);
        let mut ctx = make_ctx_with_host(Some("unknown.example.com"));
        dispatcher.handle(&mut ctx).unwrap();
        let output = take_response_string(&mut ctx);
        assert!(
            output.starts_with("HTTP/1.1 200 OK"),
            "expected 200, got: {:?}",
            output
        );
        assert!(
            output.contains("parked here"),
            "expected 'parked here' in body, got: {:?}",
            output
        );
    }

    #[test]
    fn unknown_domain_with_default_handler_dispatches_to_default() {
        let dir = make_temp_root("default_handler");
        std::fs::write(dir.join("index.html"), b"default site content").unwrap();
        let default_handler = StaticFileHandler::new(dir).unwrap();
        let dispatcher = VhostDispatcher::new(HashMap::new(), Some(default_handler));

        let mut ctx = make_ctx_with_host_and_path(Some("unknown.example.com"), "/index.html");
        dispatcher.handle(&mut ctx).unwrap();
        let output = take_response_string(&mut ctx);
        assert!(
            output.starts_with("HTTP/1.1 200 OK"),
            "expected 200 from default handler, got: {:?}",
            output
        );
        let sep_pos = output.find("\r\n\r\n").expect("no blank separator");
        let body = &output[sep_pos + 4..];
        assert!(
            body.contains("default site content"),
            "expected default handler body, got: {:?}",
            body
        );
    }

    #[test]
    fn parked_page_contains_normalized_host() {
        let dispatcher = VhostDispatcher::new(HashMap::new(), None);
        let mut ctx = make_ctx_with_host(Some("WWW.EXAMPLE.COM:8080"));
        dispatcher.handle(&mut ctx).unwrap();
        let output = take_response_string(&mut ctx);
        let sep_pos = output.find("\r\n\r\n").expect("no blank separator");
        let body = &output[sep_pos + 4..];
        // Host should be normalized: lowercase, port stripped, www. stripped
        assert!(
            body.contains("example.com"),
            "parked page should contain normalized host, got: {:?}",
            body
        );
        assert!(
            body.contains("parked here"),
            "parked page should contain 'parked here', got: {:?}",
            body
        );
    }

    #[test]
    fn absent_host_no_default_returns_parked_page() {
        let dispatcher = VhostDispatcher::new(HashMap::new(), None);
        let mut ctx = make_ctx_with_host(None);
        dispatcher.handle(&mut ctx).unwrap();
        let output = take_response_string(&mut ctx);
        assert!(
            output.starts_with("HTTP/1.1 200 OK"),
            "expected 200 for absent host, got: {:?}",
            output
        );
        assert!(
            output.contains("parked here"),
            "expected parked page for absent host, got: {:?}",
            output
        );
    }

    #[test]
    fn parked_page_has_content_type_text_html() {
        let dispatcher = VhostDispatcher::new(HashMap::new(), None);
        let mut ctx = make_ctx_with_host(Some("example.com"));
        dispatcher.handle(&mut ctx).unwrap();
        let output = take_response_string(&mut ctx);
        assert!(
            output.contains("Content-Type: text/html"),
            "expected Content-Type: text/html, got: {:?}",
            output
        );
    }
}
