//! Handler trait for HTTP request handlers.

use super::{context::Context, Result};

/// A synchronous HTTP request handler.
///
/// Handlers receive a mutable Context and write their response into `ctx.response` in place.
/// Returning `Err` causes handle_connection to send a 500 Internal Server Error response.
///
/// # Thread Safety
/// Handlers must be `Send + Sync` because they are shared across worker threads via `Arc<Router>`.
#[allow(dead_code)]
pub trait Handler: Send + Sync {
    fn handle(&self, ctx: &mut Context) -> Result<()>;
}
