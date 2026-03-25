//! HTTP request handlers.
//!
//! Each handler implements the [`server::Handler`] trait and writes its response into
//! `ctx.response` in place. Handlers are registered with the [`server::Router`] in `main.rs`.

use crate::server::{Context, Handler, RequestMethod, Response, Result};
use std::path::PathBuf;

/// Handler for `GET /` — returns 200 OK with a plain text body.
///
/// Not used in production routing (the StaticFileHandler fallback serves all paths).
/// Retained for unit tests.
#[cfg_attr(not(test), allow(dead_code))]
pub struct RootHandler;

impl Handler for RootHandler {
    fn handle(&self, ctx: &mut Context) -> Result<()> {
        let body = b"OK\n".to_vec();
        let content_length = body.len().to_string();
        ctx.response = Response::new(200, "OK")
            .header("Content-Type", "text/plain")
            .header("Content-Length", &content_length)
            .header("Connection", "close")
            .body(body);
        Ok(())
    }
}

/// Returns the MIME type for a file based on its extension.
///
/// Covers the 10 locked extensions required by FILE-02. Unknown extensions
/// fall back to `application/octet-stream`.
fn mime_type(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("html") => "text/html",
        Some("css") => "text/css",
        Some("js") => "application/javascript",
        Some("wasm") => "application/wasm",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        Some("txt") => "text/plain",
        _ => "application/octet-stream",
    }
}

/// Sets a 404 Not Found response on the context.
///
/// Used by StaticFileHandler for missing files and traversal rejections.
fn not_found(ctx: &mut Context) -> Result<()> {
    let body = b"Not Found\n".to_vec();
    let content_length = body.len().to_string();
    ctx.response = Response::new(404, "Not Found")
        .header("Content-Type", "text/plain")
        .header("Content-Length", &content_length)
        .header("Connection", "close")
        .body(body);
    Ok(())
}

/// Handler that serves static files from a configured root directory.
///
/// Implements FILE-01, FILE-02, FILE-04, FILE-05, and PATH-03:
/// - Reads files with binary-safe `fs::read()` (FILE-01, FILE-05)
/// - Detects MIME types from extension (FILE-02)
/// - Handles HEAD requests with headers only, no body (FILE-04)
/// - Applies `canonicalize + starts_with(root)` traversal guard (PATH-03)
/// - Returns 404 for missing files (not 500)
pub struct StaticFileHandler {
    canonical_root: PathBuf,
}

impl StaticFileHandler {
    /// Create a new StaticFileHandler rooted at `root`.
    ///
    /// Canonicalizes `root` at construction time so per-request checks are fast.
    /// Returns `Err` if `root` does not exist or cannot be canonicalized.
    pub fn new(root: PathBuf) -> Result<Self> {
        let canonical_root = std::fs::canonicalize(&root)?;
        Ok(StaticFileHandler { canonical_root })
    }
}

impl Handler for StaticFileHandler {
    fn handle(&self, ctx: &mut Context) -> Result<()> {
        // Decode the request path — router already rejected dotdot components.
        // An invalid %-sequence is a malformed request; treat as 404 (not 500).
        let decoded = match ctx.request.target.decoded_path() {
            Ok(d) => d,
            Err(_) => return not_found(ctx),
        };

        // Strip leading '/' before joining. PathBuf::join discards the left side if the
        // right side is absolute, so the relative form is required.
        let rel = decoded.trim_start_matches('/');
        let candidate = self.canonical_root.join(rel);

        // If the candidate path is a directory, attempt to serve index.html inside it.
        let candidate = if candidate.is_dir() {
            candidate.join("index.html")
        } else {
            candidate
        };

        // Canonicalize the candidate to resolve symlinks and any remaining traversal.
        // NotFound from canonicalize means the file doesn't exist → 404.
        let canonical = match std::fs::canonicalize(&candidate) {
            Ok(p) => p,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return not_found(ctx),
            Err(e) => return Err(e.into()),
        };

        // PATH-03: reject any path that escapes the configured root after canonicalization.
        if !canonical.starts_with(&self.canonical_root) {
            return not_found(ctx);
        }

        // Read the file with binary-safe fs::read (Vec<u8>).
        let body = match std::fs::read(&canonical) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return not_found(ctx),
            Err(e) => return Err(e.into()),
        };

        let content_type = mime_type(&canonical);
        let content_length = body.len().to_string();

        // FILE-04: HEAD returns headers only; GET returns headers + body.
        if ctx.request.method == RequestMethod::Head {
            ctx.response = Response::new(200, "OK")
                .header("Content-Type", content_type)
                .header("Content-Length", &content_length)
                .header("Connection", "close");
        } else {
            ctx.response = Response::new(200, "OK")
                .header("Content-Type", content_type)
                .header("Content-Length", &content_length)
                .header("Connection", "close")
                .body(body);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::{Context, Request, RequestMethod, Response};
    use crate::url::Url;
    use std::path::Path;

    fn make_root_ctx() -> Context {
        Context {
            request: Request {
                method: RequestMethod::Get,
                target: Url::from("/"),
            },
            response: Response::new(200, "OK"),
        }
    }

    #[test]
    fn root_handler_returns_200() {
        let mut ctx = make_root_ctx();
        RootHandler.handle(&mut ctx).unwrap();
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
    fn root_handler_has_content_type() {
        let mut ctx = make_root_ctx();
        RootHandler.handle(&mut ctx).unwrap();
        let mut buf: Vec<u8> = Vec::new();
        ctx.response.write_to(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.contains("Content-Type: text/plain"),
            "expected Content-Type header, got: {:?}",
            output
        );
    }

    #[test]
    fn root_handler_has_content_length() {
        let mut ctx = make_root_ctx();
        RootHandler.handle(&mut ctx).unwrap();
        let mut buf: Vec<u8> = Vec::new();
        ctx.response.write_to(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.contains("Content-Length:"),
            "expected Content-Length header, got: {:?}",
            output
        );
    }

    #[test]
    fn root_handler_has_connection_close() {
        let mut ctx = make_root_ctx();
        RootHandler.handle(&mut ctx).unwrap();
        let mut buf: Vec<u8> = Vec::new();
        ctx.response.write_to(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.contains("Connection: close"),
            "expected Connection: close header, got: {:?}",
            output
        );
    }

    #[test]
    fn root_handler_body_is_ok() {
        let mut ctx = make_root_ctx();
        RootHandler.handle(&mut ctx).unwrap();
        let mut buf: Vec<u8> = Vec::new();
        ctx.response.write_to(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        let sep_pos = output.find("\r\n\r\n").expect("no blank separator");
        let body = &output[sep_pos + 4..];
        assert_eq!(body, "OK\n", "expected body 'OK\\n', got: {:?}", body);
    }

    // --- Helper: make a GET context for a given path ---
    fn make_ctx(method: RequestMethod, path: &str) -> Context {
        Context {
            request: Request {
                method,
                target: Url::from(path),
            },
            response: Response::new(200, "OK"),
        }
    }

    // --- Helper: create a temp dir with a unique name ---
    fn make_temp_root(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("ptodd_test_{}", name));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    // ========== mime_type tests (unit, no filesystem) ==========

    #[test]
    fn mime_html() {
        assert_eq!(mime_type(Path::new("index.html")), "text/html");
    }

    #[test]
    fn mime_css() {
        assert_eq!(mime_type(Path::new("style.css")), "text/css");
    }

    #[test]
    fn mime_js() {
        assert_eq!(mime_type(Path::new("app.js")), "application/javascript");
    }

    #[test]
    fn mime_wasm() {
        assert_eq!(mime_type(Path::new("module.wasm")), "application/wasm");
    }

    #[test]
    fn mime_png() {
        assert_eq!(mime_type(Path::new("photo.png")), "image/png");
    }

    #[test]
    fn mime_jpg() {
        assert_eq!(mime_type(Path::new("photo.jpg")), "image/jpeg");
    }

    #[test]
    fn mime_jpeg() {
        assert_eq!(mime_type(Path::new("photo.jpeg")), "image/jpeg");
    }

    #[test]
    fn mime_gif() {
        assert_eq!(mime_type(Path::new("anim.gif")), "image/gif");
    }

    #[test]
    fn mime_svg() {
        assert_eq!(mime_type(Path::new("icon.svg")), "image/svg+xml");
    }

    #[test]
    fn mime_ico() {
        assert_eq!(mime_type(Path::new("favicon.ico")), "image/x-icon");
    }

    #[test]
    fn mime_txt() {
        assert_eq!(mime_type(Path::new("readme.txt")), "text/plain");
    }

    #[test]
    fn mime_unknown_extension() {
        assert_eq!(
            mime_type(Path::new("archive.zip")),
            "application/octet-stream"
        );
    }

    #[test]
    fn mime_no_extension() {
        assert_eq!(
            mime_type(Path::new("noextension")),
            "application/octet-stream"
        );
    }

    // ========== not_found() helper tests ==========

    #[test]
    fn not_found_sets_404_response() {
        let mut ctx = make_ctx(RequestMethod::Get, "/missing.txt");
        let result = not_found(&mut ctx);
        assert!(result.is_ok(), "not_found should return Ok(())");
        let mut buf: Vec<u8> = Vec::new();
        ctx.response.write_to(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.starts_with("HTTP/1.1 404 Not Found"),
            "expected 404, got: {:?}",
            output
        );
        let sep_pos = output.find("\r\n\r\n").expect("no blank separator");
        let body = &output[sep_pos + 4..];
        assert_eq!(body, "Not Found\n", "expected 'Not Found\\n' body");
    }

    // ========== StaticFileHandler::new() tests ==========

    #[test]
    fn new_existing_dir_returns_ok() {
        let dir = make_temp_root("new_ok");
        let result = StaticFileHandler::new(dir);
        assert!(result.is_ok(), "expected Ok for existing directory");
    }

    #[test]
    fn new_nonexistent_path_returns_err() {
        let path = std::env::temp_dir().join("ptodd_test_nonexistent_zzz_should_not_exist");
        // Ensure it doesn't exist
        let _ = std::fs::remove_dir_all(&path);
        let result = StaticFileHandler::new(path);
        assert!(result.is_err(), "expected Err for nonexistent path");
    }

    // ========== GET file serving tests (FILE-01, FILE-02, FILE-05) ==========

    #[test]
    fn get_html_file_returns_200_with_correct_content_type() {
        let dir = make_temp_root("get_html");
        std::fs::write(dir.join("index.html"), b"hello").unwrap();
        let handler = StaticFileHandler::new(dir).unwrap();
        let mut ctx = make_ctx(RequestMethod::Get, "/index.html");
        handler.handle(&mut ctx).unwrap();
        let mut buf: Vec<u8> = Vec::new();
        ctx.response.write_to(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.starts_with("HTTP/1.1 200 OK"), "expected 200");
        assert!(
            output.contains("Content-Type: text/html"),
            "expected text/html, got: {:?}",
            output
        );
        let sep_pos = output.find("\r\n\r\n").expect("no blank separator");
        let body = &output[sep_pos + 4..];
        assert_eq!(body, "hello", "expected body 'hello', got: {:?}", body);
    }

    #[test]
    fn get_binary_file_is_binary_safe() {
        let dir = make_temp_root("get_binary");
        let binary_content: &[u8] = &[0xFF, 0x89, 0x50];
        std::fs::write(dir.join("image.png"), binary_content).unwrap();
        let handler = StaticFileHandler::new(dir).unwrap();
        let mut ctx = make_ctx(RequestMethod::Get, "/image.png");
        handler.handle(&mut ctx).unwrap();
        let mut buf: Vec<u8> = Vec::new();
        ctx.response.write_to(&mut buf).unwrap();
        // Find the separator
        let sep = b"\r\n\r\n";
        let sep_pos = buf
            .windows(4)
            .position(|w| w == sep)
            .expect("no blank separator");
        let body_part = &buf[sep_pos + 4..];
        assert_eq!(
            body_part, binary_content,
            "binary body not preserved, got: {:?}",
            body_part
        );
        // Also verify content type
        let header_section = String::from_utf8(buf[..sep_pos].to_vec()).unwrap();
        assert!(
            header_section.contains("Content-Type: image/png"),
            "expected image/png, got: {:?}",
            header_section
        );
    }

    #[test]
    fn get_missing_file_returns_404_not_500() {
        let dir = make_temp_root("get_missing");
        let handler = StaticFileHandler::new(dir).unwrap();
        let mut ctx = make_ctx(RequestMethod::Get, "/missing.txt");
        let result = handler.handle(&mut ctx);
        assert!(result.is_ok(), "missing file must return Ok(()), not Err");
        let mut buf: Vec<u8> = Vec::new();
        ctx.response.write_to(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.starts_with("HTTP/1.1 404"),
            "expected 404 for missing file, got: {:?}",
            output
        );
        assert!(
            output.contains("Not Found"),
            "expected 'Not Found' in body, got: {:?}",
            output
        );
    }

    #[test]
    fn get_unknown_extension_returns_octet_stream() {
        let dir = make_temp_root("get_unknown_ext");
        std::fs::write(dir.join("unknown.xyz"), b"data").unwrap();
        let handler = StaticFileHandler::new(dir).unwrap();
        let mut ctx = make_ctx(RequestMethod::Get, "/unknown.xyz");
        handler.handle(&mut ctx).unwrap();
        let mut buf: Vec<u8> = Vec::new();
        ctx.response.write_to(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(output.starts_with("HTTP/1.1 200 OK"), "expected 200");
        assert!(
            output.contains("Content-Type: application/octet-stream"),
            "expected octet-stream, got: {:?}",
            output
        );
    }

    // ========== HEAD request tests (FILE-04) ==========

    #[test]
    fn head_request_returns_headers_only_no_body() {
        let dir = make_temp_root("head_request");
        std::fs::write(dir.join("index.html"), b"hello").unwrap();
        let handler = StaticFileHandler::new(dir).unwrap();
        let mut ctx = make_ctx(RequestMethod::Head, "/index.html");
        handler.handle(&mut ctx).unwrap();
        let mut buf: Vec<u8> = Vec::new();
        ctx.response.write_to(&mut buf).unwrap();
        // Find the blank separator
        let sep = b"\r\n\r\n";
        let sep_pos = buf
            .windows(4)
            .position(|w| w == sep)
            .expect("no blank separator");
        let body_after_separator = &buf[sep_pos + 4..];
        assert!(
            body_after_separator.is_empty(),
            "HEAD response must have no body bytes, got: {:?}",
            body_after_separator
        );
        // But headers must be present
        let header_section = String::from_utf8(buf[..sep_pos].to_vec()).unwrap();
        assert!(
            header_section.contains("HTTP/1.1 200 OK"),
            "expected 200 status, got: {:?}",
            header_section
        );
        assert!(
            header_section.contains("Content-Type: text/html"),
            "expected Content-Type header, got: {:?}",
            header_section
        );
        assert!(
            header_section.contains("Content-Length: 5"),
            "expected Content-Length: 5, got: {:?}",
            header_section
        );
    }

    // ========== PATH-03 canonicalize traversal tests ==========

    #[test]
    #[cfg(unix)]
    fn symlink_escaping_root_returns_404() {
        // Create tmpdir/root/ and tmpdir/outside.txt
        let base = make_temp_root("path03_symlink");
        let root = base.join("root");
        std::fs::create_dir_all(&root).unwrap();
        let outside = base.join("outside.txt");
        std::fs::write(&outside, b"secret").unwrap();
        // Create symlink: root/link -> ../outside.txt (escapes root)
        // Remove pre-existing symlink from previous test runs to ensure idempotent setup.
        let link_path = root.join("link");
        let _ = std::fs::remove_file(&link_path);
        std::os::unix::fs::symlink(&outside, &link_path).unwrap();
        let handler = StaticFileHandler::new(root).unwrap();
        let mut ctx = make_ctx(RequestMethod::Get, "/link");
        let result = handler.handle(&mut ctx);
        // Must return Ok(()), not Err
        assert!(
            result.is_ok(),
            "symlink traversal must return Ok(()), not Err"
        );
        let mut buf: Vec<u8> = Vec::new();
        ctx.response.write_to(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.starts_with("HTTP/1.1 404"),
            "symlink escaping root must return 404, got: {:?}",
            output
        );
    }
}
