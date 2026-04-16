//! Plugin metadata trait for prefix-routed request handlers.

use super::handler::Handler;

/// Metadata extension for prefix-routed plugins.
///
/// Plugins implement this trait instead of Handler directly. Because KissPlugin
/// extends Handler, any KissPlugin is also a Handler and can be stored in the
/// router's `Box<dyn Handler>` slots.
///
/// # Object Safety
/// Both methods take `&self` and return `&str` — fully object-safe.
/// `Box<dyn KissPlugin>` is valid.
///
/// # Thread Safety
/// Inherits `Send + Sync` from the `Handler` supertrait. Plugin state
/// held in `Arc<RwLock<T>>` satisfies both bounds without unsafe.
#[allow(dead_code)]
pub trait KissPlugin: Handler {
    /// Human-readable plugin identifier, used in startup logs and error messages.
    fn name(&self) -> &str;
    /// URL prefix this plugin owns. Requests whose decoded path starts_with this
    /// prefix are routed to this plugin.
    fn path_prefix(&self) -> &str;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::{context::Context, Result};
    use std::collections::HashMap;
    use std::sync::{Arc, RwLock};

    /// Compile-time proof that Arc<RwLock<HashMap>> satisfies Send + Sync
    /// via the Handler + KissPlugin supertrait bounds (PLUG-05).
    struct StatefulPlugin {
        store: Arc<RwLock<HashMap<String, String>>>,
    }

    impl Handler for StatefulPlugin {
        fn handle(&self, ctx: &mut Context) -> Result<()> {
            let _guard = self.store.read().unwrap();
            ctx.response = crate::server::Response::new(200, "OK")
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

    #[test]
    fn stateful_plugin_is_send_and_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<StatefulPlugin>();
    }

    #[test]
    fn stateful_plugin_compiles_as_box_dyn_handler() {
        let plugin = StatefulPlugin {
            store: Arc::new(RwLock::new(HashMap::new())),
        };
        // Must compile: KissPlugin is a Handler, so Box<dyn Handler> works
        let _boxed: Box<dyn Handler> = Box::new(plugin);
    }
}
