//! Plugin metadata trait -- re-exported from kiss-plugin-sdk.
pub use kiss_plugin_sdk::KissPlugin;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::handler::Handler;
    use kiss_plugin_sdk::{Context, Result};
    use std::collections::HashMap;
    use std::sync::{Arc, RwLock};

    /// Compile-time proof that Arc<RwLock<HashMap>> satisfies Send and Sync
    /// via the Handler and KissPlugin super trait bounds (PLUG-05).
    struct StatefulPlugin {
        store: Arc<RwLock<HashMap<String, String>>>,
    }

    impl Handler for StatefulPlugin {
        fn handle(&self, ctx: &mut Context) -> Result<()> {
            let _guard = self.store.read().unwrap();
            ctx.response = kiss_plugin_sdk::Response::new(200, "OK")
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
