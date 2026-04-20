//! A from-scratch HTTP/1.1 static file server written in pure Rust. Runs behind CloudFront for TLS
//! termination at ptodd.org / www.ptodd.org. Supports multi-domain virtual hosting via `--config`
//! or single-root via `--root`.

use kiss_plugin_sdk::KissPlugin;
use kiss_url_shortener::UrlShortener;
use log::info;

use logger::SimpleLogger;
use server::{AuthMiddleware, MiddlewareChain, Router, Server};

mod args;
mod config;
mod handlers;
mod logger;
mod server;
mod time;
mod url;

const DEFAULT_PORT: u16 = 6502;

pub type Error = Box<dyn std::error::Error>;
pub type Result<T> = std::result::Result<T, Error>;

/// Builds a [`handlers::VhostDispatcher`] from a parsed arg map, returning it alongside any
/// plugin configs parsed from the TOML config file.
///
/// - `--config <path>`: loads TOML config and creates per-domain handlers.
/// - `--root <path>`: synthesizes a dispatcher with a single default handler (backward compat).
/// - Both flags together: returns an error.
/// - Neither flag: returns an error.
fn build_dispatcher(
    parsed: &std::collections::HashMap<String, Option<String>>,
) -> Result<(handlers::VhostDispatcher, Vec<config::PluginConfig>)> {
    let has_config = args::has_flag(parsed, "--config");
    let has_root = args::has_flag(parsed, "--root");

    if has_config && has_root {
        return Err("--config and --root are mutually exclusive".into());
    }

    if has_config {
        let config_path = args::get_path(parsed, "--config", args::PathKind::File)?;
        let config = config::Config::load(&config_path)
            .map_err(|e| format!("failed to load config: {}", e))?;

        info!(
            "Loaded config from {}: {} vhost(s)",
            config_path.display(),
            config.vhosts.len()
        );
        for entry in &config.vhosts {
            info!("  vhost: {} -> {}", entry.domain, entry.root);
        }

        let mut vhosts = std::collections::HashMap::new();
        for entry in &config.vhosts {
            vhosts.insert(
                entry.domain.clone(),
                handlers::StaticFileHandler::new(std::path::PathBuf::from(&entry.root))
                    .map_err(|e| format!("vhost '{}': {}", entry.domain, e))?,
            );
        }

        let default_handler = config
            .server
            .default_root
            .as_ref()
            .map(|root| {
                let path = std::path::PathBuf::from(root);
                handlers::StaticFileHandler::new(path)
                    .map_err(|e| format!("default_root '{}': {}", root, e))
            })
            .transpose()?;

        Ok((
            handlers::VhostDispatcher::new(vhosts, default_handler),
            config.plugins,
        ))
    } else if has_root {
        let root = args::get_path(parsed, "--root", args::PathKind::Dir)?;
        info!("Serving static files from root: {}", root.display());
        Ok((
            handlers::VhostDispatcher::new(
                std::collections::HashMap::new(),
                Some(handlers::StaticFileHandler::new(root)?),
            ),
            Vec::new(),
        ))
    } else {
        Err("either --config <path> or --root <path> is required".into())
    }
}

fn main() -> Result<()> {
    SimpleLogger::init()?;
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    let parsed_args = args::parse(&raw_args);
    let port = args::get_parsed::<u16>(&parsed_args, "--port", DEFAULT_PORT)?;
    let addr = format!("0.0.0.0:{}", port);
    let (dispatcher, plugin_configs) = build_dispatcher(&parsed_args)?;
    let mut router = Router::new().set_fallback(dispatcher);

    if !plugin_configs.is_empty() {
        info!("{} plugin(s) configured", plugin_configs.len());
    }

    // Plugin activation: map each configured plugin name to its constructor (PLUG-03, D-03, D-08).
    for plugin_config in &plugin_configs {
        match plugin_config.name.as_str() {
            "url-shortener" => {
                let sdk_cfg = kiss_plugin_sdk::PluginConfig {
                    name: plugin_config.name.clone(),
                    extra: plugin_config.extra.clone(),
                };
                let p = UrlShortener::new(&sdk_cfg);
                let prefix = p.path_prefix().to_string();
                let name = p.name().to_string();
                info!("  plugin: {} -> {}", name, prefix);
                router.add_prefix(prefix, p)?;
            }
            other => {
                return Err(format!(
                    "unknown plugin '{}': not registered in main.rs; \
                     add a match arm or remove the [[plugin]] block from kiss-server.toml",
                    other
                )
                .into());
            }
        }
    }

    let skip_auth = std::env::var("KISS_SKIP_AUTH")
        .map(|v| !matches!(v.to_ascii_lowercase().as_str(), "0" | "false" | "no"))
        .unwrap_or(false);
    let middleware_chain = if skip_auth {
        info!("KISS_SKIP_AUTH set — auth middleware disabled (dev mode only)");
        MiddlewareChain::new().public_routes(&[])
    } else {
        MiddlewareChain::new()
            .add(AuthMiddleware::new())
            .public_routes(&["/health", "/favicon.ico"])
    };

    Server::new(&addr)?
        .with_router(router)
        .with_middleware(middleware_chain)
        .run()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    // ========== build_dispatcher() unit tests ==========

    #[test]
    fn build_dispatcher_both_flags_returns_err() {
        let temp_dir = std::env::temp_dir().to_string_lossy().to_string();
        let raw_args = vec![
            "--config".to_string(),
            "/some/file.toml".to_string(),
            "--root".to_string(),
            temp_dir,
        ];
        let parsed = args::parse(&raw_args);
        let result = build_dispatcher(&parsed);
        assert!(
            result.is_err(),
            "expected Err when both --config and --root present"
        );
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("mutually exclusive"),
            "error should mention 'mutually exclusive', got: {:?}",
            msg
        );
    }

    #[test]
    fn build_dispatcher_neither_flag_returns_err() {
        let raw_args: Vec<String> = vec![];
        let parsed = args::parse(&raw_args);
        let result = build_dispatcher(&parsed);
        assert!(
            result.is_err(),
            "expected Err when neither --config nor --root present"
        );
    }

    #[test]
    fn build_dispatcher_root_returns_dispatcher() {
        let temp_dir = std::env::temp_dir().to_string_lossy().to_string();
        let raw_args = vec!["--root".to_string(), temp_dir];
        let parsed = args::parse(&raw_args);
        let result = build_dispatcher(&parsed);
        assert!(
            result.is_ok(),
            "expected Ok with valid --root, got: {:?}",
            result
        );
    }

    #[test]
    fn build_dispatcher_config_builds_per_domain_handlers() {
        // Create a temp vhost root directory
        let temp_dir = std::env::temp_dir();
        let vhost_root = temp_dir.join("kiss_test_vhost_root");
        std::fs::create_dir_all(&vhost_root).unwrap();

        // Write a valid TOML config file referencing that directory
        let config_file = temp_dir.join("kiss_test_vhost.toml");
        let toml = format!(
            "[[vhost]]\ndomain = \"example.com\"\nroot = \"{}\"\n",
            vhost_root.display()
        );
        let mut f = std::fs::File::create(&config_file).unwrap();
        f.write_all(toml.as_bytes()).unwrap();
        drop(f);

        let raw_args = vec![
            "--config".to_string(),
            config_file.to_string_lossy().to_string(),
        ];
        let parsed = args::parse(&raw_args);
        let result = build_dispatcher(&parsed);

        let _ = std::fs::remove_file(&config_file);
        let _ = std::fs::remove_dir(&vhost_root);

        assert!(
            result.is_ok(),
            "expected Ok with valid --config, got: {:?}",
            result
        );
    }
}
