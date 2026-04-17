//! Request/response pipeline context, threaded through the handler chain.

use super::{request::Request, response::Response};

/// Authentication identity extracted by middleware. Minimum viable: user_id only.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct AuthClaims {
    pub user_id: String,
}

/// Shared mutable state for a single HTTP request/response cycle.
///
/// Constructed per-request in handle_connection. The default response is 200 OK.
/// Handlers overwrite `ctx.response` in place via the Response builder.
pub struct Context {
    pub request: Request,
    pub response: Response,
    #[allow(dead_code)]
    pub auth: Option<AuthClaims>,
}
