//! URL shortener plugin for kiss-server.

use kiss_plugin_sdk::{Context, Handler, KissPlugin, PluginConfig, Response, Result};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// A URL shortener plugin that redirects /s/{code} to stored target URLs.
///
/// Holds in-memory redirect state in Arc<RwLock<HashMap>> (PLUG-05).
/// Seed data is hardcoded in new() (D-06). State resets on restart.
pub struct UrlShortener {
    store: Arc<RwLock<HashMap<String, String>>>,
}

impl UrlShortener {
    /// Create a new UrlShortener with hardcoded seed data.
    pub fn new(_config: &PluginConfig) -> Self {
        let mut map = HashMap::new();
        map.insert("gh".to_string(), "https://github.com/ptdecker".to_string());
        map.insert("rs".to_string(), "https://www.rust-lang.org".to_string());
        map.insert("hn".to_string(), "https://news.ycombinator.com".to_string());
        UrlShortener {
            store: Arc::new(RwLock::new(map)),
        }
    }
}

impl Handler for UrlShortener {
    fn handle(&self, ctx: &mut Context) -> Result<()> {
        // Extract short code by stripping the plugin's own prefix + "/".
        let prefix_slash = format!("{}/", self.path_prefix());
        let code = ctx
            .request
            .target
            .decoded_path()
            .ok()
            .and_then(|p| p.strip_prefix(prefix_slash.as_str()).map(str::to_string));

        let store = self.store.read().unwrap();
        match code.and_then(|c| store.get(&c).cloned()) {
            Some(url) => {
                // D-05: 302 Found (not 301 -- in-memory state is volatile)
                ctx.response = Response::new(302, "Found")
                    .header("Location", &url)
                    .header("Content-Length", "0")
                    .header("Connection", "close")
                    .body(vec![]);
            }
            None => {
                // D-07: Content-negotiated 404
                let accept = ctx.request.header("accept").unwrap_or("");
                if accept.contains("application/json") {
                    let body = b"{\"error\":\"short code not found\"}".to_vec();
                    ctx.response = Response::new(404, "Not Found")
                        .header("Content-Type", "application/json")
                        .header("Content-Length", &body.len().to_string())
                        .header("Connection", "close")
                        .body(body);
                } else if accept.contains("text/html") {
                    let body =
                        b"<!DOCTYPE html>\n<html><body><p>Short code not found</p></body></html>"
                            .to_vec();
                    ctx.response = Response::new(404, "Not Found")
                        .header("Content-Type", "text/html")
                        .header("Content-Length", &body.len().to_string())
                        .header("Connection", "close")
                        .body(body);
                } else {
                    let body = b"Short code not found\n".to_vec();
                    ctx.response = Response::new(404, "Not Found")
                        .header("Content-Type", "text/plain")
                        .header("Content-Length", &body.len().to_string())
                        .header("Connection", "close")
                        .body(body);
                }
            }
        }
        Ok(())
    }
}

impl KissPlugin for UrlShortener {
    fn name(&self) -> &str {
        "url-shortener"
    }
    fn path_prefix(&self) -> &str {
        "/s"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kiss_plugin_sdk::test_support::{test_context, test_context_with_headers};

    fn make_plugin() -> UrlShortener {
        UrlShortener::new(&PluginConfig {
            name: "url-shortener".to_string(),
            extra: Default::default(),
        })
    }

    // Checklist item 7: handle() testable in 5 lines
    #[test]
    fn handle_testable_in_five_lines() {
        let plugin = make_plugin();
        let mut ctx = test_context("GET", "/s/gh");
        plugin.handle(&mut ctx).unwrap();
        assert_eq!(ctx.response.status(), 302);
    }

    // Checklist item 3: Arc<RwLock<HashMap>> compiles as Send + Sync
    #[test]
    fn url_shortener_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<UrlShortener>();
    }

    // D-05: known code returns 302 with a Location header
    #[test]
    fn known_code_returns_302_with_location() {
        let plugin = make_plugin();
        let mut ctx = test_context("GET", "/s/gh");
        plugin.handle(&mut ctx).unwrap();
        assert_eq!(ctx.response.status(), 302);
        // Verify Location header by writing a response and checking output
        let mut buf: Vec<u8> = Vec::new();
        ctx.response.write_to(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.contains("Location: https://github.com/ptdecker"),
            "expected Location header, got: {:?}",
            output
        );
    }

    // D-05: another seed code works
    #[test]
    fn known_code_rs_returns_302() {
        let plugin = make_plugin();
        let mut ctx = test_context("GET", "/s/rs");
        plugin.handle(&mut ctx).unwrap();
        assert_eq!(ctx.response.status(), 302);
    }

    // D-07: unknown code returns 404 plain text (default)
    #[test]
    fn unknown_code_returns_404_plain_text() {
        let plugin = make_plugin();
        let mut ctx = test_context("GET", "/s/nonexistent");
        plugin.handle(&mut ctx).unwrap();
        assert_eq!(ctx.response.status(), 404);
        let mut buf: Vec<u8> = Vec::new();
        ctx.response.write_to(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.contains("Content-Type: text/plain"),
            "expected text/plain, got: {:?}",
            output
        );
        assert!(
            output.contains("Short code not found"),
            "expected body text, got: {:?}",
            output
        );
    }

    // D-07: unknown code with Accept: application/json returns JSON 404
    #[test]
    fn unknown_code_returns_404_json() {
        let plugin = make_plugin();
        let mut ctx = test_context_with_headers("GET", "/s/bad", &[("Accept", "application/json")]);
        plugin.handle(&mut ctx).unwrap();
        assert_eq!(ctx.response.status(), 404);
        let mut buf: Vec<u8> = Vec::new();
        ctx.response.write_to(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.contains("Content-Type: application/json"),
            "expected JSON content-type, got: {:?}",
            output
        );
        assert!(
            output.contains("\"error\""),
            "expected JSON error key, got: {:?}",
            output
        );
    }

    // D-07: unknown code with Accept: text/html returns HTML 404
    #[test]
    fn unknown_code_returns_404_html() {
        let plugin = make_plugin();
        let mut ctx = test_context_with_headers("GET", "/s/bad", &[("Accept", "text/html")]);
        plugin.handle(&mut ctx).unwrap();
        assert_eq!(ctx.response.status(), 404);
        let mut buf: Vec<u8> = Vec::new();
        ctx.response.write_to(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.contains("Content-Type: text/html"),
            "expected HTML content-type, got: {:?}",
            output
        );
        assert!(
            output.contains("<p>Short code not found</p>"),
            "expected HTML body, got: {:?}",
            output
        );
    }

    // Bare /s (no trailing code) returns 404
    #[test]
    fn bare_prefix_returns_404() {
        let plugin = make_plugin();
        let mut ctx = test_context("GET", "/s");
        plugin.handle(&mut ctx).unwrap();
        assert_eq!(ctx.response.status(), 404);
    }

    // /s/ (trailing slash, empty code) returns 404
    #[test]
    fn empty_code_returns_404() {
        let plugin = make_plugin();
        let mut ctx = test_context("GET", "/s/");
        plugin.handle(&mut ctx).unwrap();
        assert_eq!(ctx.response.status(), 404);
    }

    // KissPlugin metadata
    #[test]
    fn plugin_name_is_url_shortener() {
        let plugin = make_plugin();
        assert_eq!(plugin.name(), "url-shortener");
    }

    #[test]
    fn plugin_path_prefix_is_s() {
        let plugin = make_plugin();
        assert_eq!(plugin.path_prefix(), "/s");
    }
}
