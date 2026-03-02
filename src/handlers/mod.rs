//! HTTP request handlers.
//!
//! Each handler implements the [`server::Handler`] trait and writes its response into
//! `ctx.response` in place. Handlers are registered with the [`server::Router`] in `main.rs`.

use crate::server::{Context, Handler, Response, Result};

/// Handler for `GET /` — returns 200 OK with a plain text body.
pub struct RootHandler;

impl Handler for RootHandler {
    fn handle(&self, ctx: &mut Context) -> Result<()> {
        let body = b"OK".to_vec();
        let content_length = body.len().to_string();
        ctx.response = Response::new(200, "OK")
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
    use crate::server::{Context, Request, RequestMethod, Response};
    use crate::url::Url;

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
        assert_eq!(body, "OK", "expected body 'OK', got: {:?}", body);
    }
}
