//! A from-scratch HTTP/1.1 static file server written in pure Rust.
//! Runs behind CloudFront for TLS termination at ptodd.org / www.ptodd.org.
//! Supports multi-domain virtual hosting via `--config` or single-root via `--root`.

use kiss_plugin_sdk::KissPlugin;
use kiss_url_shortener::UrlShortener;
use log::info;

use logger::SimpleLogger;
use server::{AuthMiddleware, MiddlewareChain, Router, Server};

mod config;
mod handlers;
mod logger;
mod server;
mod time;
mod url;

const DEFAULT_PORT: u16 = 6502;

pub type Error = Box<dyn std::error::Error>;
pub type Result<T> = std::result::Result<T, Error>;

/// Parses `--root <path>` from the provided args slice (excluding the binary name at index 0).
///
/// Returns the validated path (confirms it is a directory before returning).
/// `StaticFileHandler::new()` will canonicalize it further.
fn parse_root_from(args: &[String]) -> Result<std::path::PathBuf> {
    if let Some(pos) = args.iter().position(|a| a == "--root") {
        let path_str = args.get(pos + 1).ok_or("--root requires a path argument")?;
        let path = std::path::PathBuf::from(path_str);
        if !path.is_dir() {
            return Err(format!("--root '{}': not a directory or does not exist", path_str).into());
        }
        Ok(path)
    } else {
        Err("--root <path> is required".into())
    }
}

/// Parses `--config <path>` from the provided args slice (excluding the binary name at index 0).
///
/// Returns the validated path (confirms it is a file before returning).
/// `Config::load()` will parse the TOML content.
fn parse_config_from(args: &[String]) -> Result<std::path::PathBuf> {
    if let Some(pos) = args.iter().position(|a| a == "--config") {
        let path_str = args
            .get(pos + 1)
            .ok_or("--config requires a path argument")?;
        let path = std::path::PathBuf::from(path_str);
        if !path.is_file() {
            return Err(format!("--config '{}': not a file or does not exist", path_str).into());
        }
        Ok(path)
    } else {
        Err("--config <path> is required".into())
    }
}

/// Parses `--port <num>` from the provided args slice (excluding the binary name at index 0).
///
/// Returns the port number as `u16`, or `DEFAULT_PORT` if `--port` is absent.
fn parse_port_from(args: &[String]) -> Result<u16> {
    if let Some(pos) = args.iter().position(|a| a == "--port") {
        let port_str = args.get(pos + 1).ok_or("--port requires a port number")?;
        let port: u16 = port_str
            .parse()
            .map_err(|_| format!("--port '{}': not a valid port number", port_str))?;
        Ok(port)
    } else {
        Ok(DEFAULT_PORT)
    }
}

/// Builds a [`handlers::VhostDispatcher`] from CLI args, returning it alongside any
/// plugin configs parsed from the TOML config file.
///
/// - `--config <path>`: loads TOML config and creates per-domain handlers.
/// - `--root <path>`: synthesizes a dispatcher with a single default handler (backward compat).
/// - Both flags together: returns an error.
/// - Neither flag: returns an error.
fn build_dispatcher(
    args: &[String],
) -> Result<(handlers::VhostDispatcher, Vec<config::PluginConfig>)> {
    let has_config = args.iter().any(|a| a == "--config");
    let has_root = args.iter().any(|a| a == "--root");

    if has_config && has_root {
        return Err("--config and --root are mutually exclusive".into());
    }

    if has_config {
        let config_path = parse_config_from(args)?;
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
            let root = std::path::PathBuf::from(&entry.root);
            let handler = handlers::StaticFileHandler::new(root)
                .map_err(|e| format!("vhost '{}': {}", entry.domain, e))?;
            vhosts.insert(entry.domain.clone(), handler);
        }

        let default_handler = match &config.server.default_root {
            Some(root) => {
                let path = std::path::PathBuf::from(root);
                Some(
                    handlers::StaticFileHandler::new(path)
                        .map_err(|e| format!("default_root '{}': {}", root, e))?,
                )
            }
            None => None,
        };

        Ok((
            handlers::VhostDispatcher::new(vhosts, default_handler),
            config.plugins,
        ))
    } else if has_root {
        let root = parse_root_from(args)?;
        info!("Serving static files from root: {}", root.display());
        let handler = handlers::StaticFileHandler::new(root)?;
        Ok((
            handlers::VhostDispatcher::new(std::collections::HashMap::new(), Some(handler)),
            Vec::new(),
        ))
    } else {
        Err("either --config <path> or --root <path> is required".into())
    }
}

fn main() -> Result<()> {
    SimpleLogger::init()?;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let port = parse_port_from(&args)?;
    let addr = format!("0.0.0.0:{}", port);
    let (dispatcher, plugin_configs) = build_dispatcher(&args)?;
    let mut router = Router::new().set_fallback(dispatcher);

    if !plugin_configs.is_empty() {
        info!("{} plugin(s) configured", plugin_configs.len());
    }

    // Plugin activation: map each configured plugin name to its constructor (PLUG-03, D-03, D-08).
    for cfg in &plugin_configs {
        match cfg.name.as_str() {
            "url-shortener" => {
                let sdk_cfg = kiss_plugin_sdk::PluginConfig {
                    name: cfg.name.clone(),
                    extra: cfg.extra.clone(),
                };
                let p = UrlShortener::new(&sdk_cfg);
                let prefix = p.path_prefix().to_string();
                let name = p.name().to_string();
                info!("  plugin: {} -> {}", name, prefix);
                router.add_prefix(prefix, p);
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

    let middleware_chain = MiddlewareChain::new()
        .add(AuthMiddleware::new())
        .public_routes(&["/health", "/favicon.ico"]);

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

    // ========== parse_config_from() unit tests ==========

    #[test]
    fn parse_config_from_valid_file_returns_ok() {
        let dir = std::env::temp_dir();
        let file_path = dir.join("kiss_test_config_valid.toml");
        std::fs::write(&file_path, b"").unwrap();
        let path_str = file_path.to_string_lossy().to_string();
        let args = vec!["--config".to_string(), path_str];
        let result = parse_config_from(&args);
        let _ = std::fs::remove_file(&file_path);
        assert!(
            result.is_ok(),
            "expected Ok for existing file, got: {:?}",
            result
        );
    }

    #[test]
    fn parse_config_from_no_config_flag_returns_err() {
        let args: Vec<String> = vec![];
        let result = parse_config_from(&args);
        assert!(result.is_err(), "expected Err when --config is absent");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("--config"),
            "error should mention --config, got: {:?}",
            msg
        );
    }

    #[test]
    fn parse_config_from_missing_path_value_returns_err() {
        let args = vec!["--config".to_string()];
        let result = parse_config_from(&args);
        assert!(
            result.is_err(),
            "expected Err when --config has no following path"
        );
    }

    #[test]
    fn parse_config_from_nonexistent_path_returns_err() {
        let args = vec![
            "--config".to_string(),
            "/nonexistent/path/that/should/never/exist.toml".to_string(),
        ];
        let result = parse_config_from(&args);
        assert!(result.is_err(), "expected Err for nonexistent path");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("not a file"),
            "error should say 'not a file', got: {:?}",
            msg
        );
    }

    #[test]
    fn parse_config_from_directory_path_returns_err() {
        let dir = std::env::temp_dir().to_string_lossy().to_string();
        let args = vec!["--config".to_string(), dir];
        let result = parse_config_from(&args);
        assert!(result.is_err(), "expected Err when path is a directory");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("not a file"),
            "error should say 'not a file', got: {:?}",
            msg
        );
    }

    // ========== build_dispatcher() unit tests ==========

    #[test]
    fn build_dispatcher_both_flags_returns_err() {
        let temp_dir = std::env::temp_dir().to_string_lossy().to_string();
        let args = vec![
            "--config".to_string(),
            "/some/file.toml".to_string(),
            "--root".to_string(),
            temp_dir,
        ];
        let result = build_dispatcher(&args);
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
        let args: Vec<String> = vec![];
        let result = build_dispatcher(&args);
        assert!(
            result.is_err(),
            "expected Err when neither --config nor --root present"
        );
    }

    #[test]
    fn build_dispatcher_root_returns_dispatcher() {
        let temp_dir = std::env::temp_dir().to_string_lossy().to_string();
        let args = vec!["--root".to_string(), temp_dir];
        let result = build_dispatcher(&args);
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

        let args = vec![
            "--config".to_string(),
            config_file.to_string_lossy().to_string(),
        ];
        let result = build_dispatcher(&args);

        let _ = std::fs::remove_file(&config_file);
        let _ = std::fs::remove_dir(&vhost_root);

        assert!(
            result.is_ok(),
            "expected Ok with valid --config, got: {:?}",
            result
        );
    }

    // ========== parse_root_from() unit tests ==========

    #[test]
    fn parse_root_from_valid_dir_returns_ok() {
        // Use an existing directory — std::env::temp_dir() always exists.
        let temp = std::env::temp_dir().to_string_lossy().to_string();
        let args = vec!["--root".to_string(), temp];
        let result = parse_root_from(&args);
        assert!(
            result.is_ok(),
            "expected Ok for existing directory, got: {:?}",
            result
        );
    }

    #[test]
    fn parse_root_from_no_root_flag_returns_err() {
        let args: Vec<String> = vec![];
        let result = parse_root_from(&args);
        assert!(
            result.is_err(),
            "expected Err when --root is absent, got: {:?}",
            result
        );
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("--root"),
            "error message should mention --root, got: {:?}",
            msg
        );
    }

    #[test]
    fn parse_root_from_root_flag_missing_path_value_returns_err() {
        let args = vec!["--root".to_string()];
        let result = parse_root_from(&args);
        assert!(
            result.is_err(),
            "expected Err when --root has no following path"
        );
    }

    #[test]
    fn parse_root_from_nonexistent_path_returns_err_with_root_in_message() {
        let args = vec![
            "--root".to_string(),
            "/nonexistent/path/that/should/never/exist".to_string(),
        ];
        let result = parse_root_from(&args);
        assert!(result.is_err(), "expected Err for nonexistent path");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("--root"),
            "error message should mention --root, got: {:?}",
            msg
        );
    }

    // ========== parse_port_from() unit tests ==========

    #[test]
    fn parse_port_from_valid_port_returns_ok() {
        let args = vec!["--port".to_string(), "8080".to_string()];
        let result = parse_port_from(&args);
        assert!(
            result.is_ok(),
            "expected Ok(8080) for valid port, got: {:?}",
            result
        );
        assert_eq!(result.unwrap(), 8080u16);
    }

    #[test]
    fn parse_port_from_no_port_flag_returns_default() {
        let args: Vec<String> = vec![];
        let result = parse_port_from(&args);
        assert!(
            result.is_ok(),
            "expected Ok(DEFAULT_PORT) when --port is absent, got: {:?}",
            result
        );
        assert_eq!(result.unwrap(), DEFAULT_PORT);
    }

    #[test]
    fn parse_port_from_invalid_port_value_returns_err() {
        let args = vec!["--port".to_string(), "abc".to_string()];
        let result = parse_port_from(&args);
        assert!(
            result.is_err(),
            "expected Err for invalid port value, got: {:?}",
            result
        );
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("--port"),
            "error message should mention --port, got: {:?}",
            msg
        );
    }
}
