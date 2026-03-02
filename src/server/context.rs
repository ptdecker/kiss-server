//! Request/response pipeline context, threaded through the handler chain.

use super::{request::Request, response::Response};

/// Shared mutable state for a single HTTP request/response cycle.
///
/// Constructed per-request in handle_connection. The default response is 200 OK.
/// Handlers overwrite `ctx.response` in place via the Response builder.
#[allow(dead_code)]
pub struct Context {
    pub request: Request,
    pub response: Response,
}
