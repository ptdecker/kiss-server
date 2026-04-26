//! A from-scratch HTTP/1.1 static file server written in pure Rust. Runs behind CloudFront for TLS
//! termination at ptodd.org / www.ptodd.org. Supports multi-domain virtual hosting via `--config`
//! or single-root via `--root`.

use kiss_plugin_sdk::KissPlugin;
use kiss_url_shortener::UrlShortener;
use log::info;

use logger::SimpleLogger;
use server::{MiddlewareChain, Router, Server};

mod args;
mod base64; // Phase 24 Plan 01
mod config;
mod handlers;
mod jwks;
mod jwt; // Phase 24 Plan 01
mod logger;
mod server;
mod time;
mod url;

const DEFAULT_PORT: u16 = 6502;

pub type Error = Box<dyn std::error::Error>;
pub type Result<T> = std::result::Result<T, Error>;

/// Builds a [`handlers::VhostDispatcher`] from a parsed arg map, returning it alongside any
/// plugin configs parsed from the TOML config file, and the parsed [`config::Config`] if
/// `--config` was used (returns `None` for the config in `--root` mode).
///
/// - `--config <path>`: loads TOML config and creates per-domain handlers.
/// - `--root <path>`: synthesizes a dispatcher with a single default handler (backward compat).
/// - Both flags together: returns an error.
/// - Neither flag: returns an error.
fn build_dispatcher(
    parsed: &std::collections::HashMap<String, Option<String>>,
) -> Result<(
    handlers::VhostDispatcher,
    Vec<config::PluginConfig>,
    Option<config::Config>,
)> {
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

        let plugins = config.plugins.clone();
        Ok((
            handlers::VhostDispatcher::new(vhosts, default_handler),
            plugins,
            Some(config),
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
            None,
        ))
    } else {
        Err("either --config <path> or --root <path> is required".into())
    }
}

/// Build an AuthMiddleware from the parsed config, or return None if auth is
/// not configured (preserves v1.5.x backward compatibility — operators who haven't
/// added auth fields to kiss-server.toml see no behavior change).
///
/// Auth requires THREE [server] fields together: jwks_url, issuer, audience.
/// Each vhost may opt-in independently with login_url + public_paths.
/// Mixing presence/absence of these fields produces a clear startup error
/// (per ACFG-04 — fail loud at parse time, not at first request).
fn build_auth_middleware(config: &config::Config) -> Result<Option<server::AuthMiddleware>> {
    // Detect partial server-level auth config (all three or none)
    let server_auth_keys = [
        ("jwks_url", config.server.jwks_url.as_deref()),
        ("issuer", config.server.issuer.as_deref()),
        ("audience", config.server.audience.as_deref()),
    ];
    let present_count = server_auth_keys.iter().filter(|(_, v)| v.is_some()).count();

    // Detect any vhost-level auth (login_url is the marker; public_paths consistency
    // was enforced at parse time in commit_vhost — Plan 01 Task 2)
    let any_vhost_has_auth = config.vhosts.iter().any(|v| v.login_url.is_some());

    // Case A: nothing configured — auth fully disabled (v1.5.x compat)
    if present_count == 0 && !any_vhost_has_auth {
        info!("Auth disabled: no [server] jwks_url/issuer/audience and no vhost login_url");
        return Ok(None);
    }

    // Case B: partial server auth config — fail loud
    if present_count != 0 && present_count != 3 {
        let missing: Vec<&str> = server_auth_keys
            .iter()
            .filter(|(_, v)| v.is_none())
            .map(|(k, _)| *k)
            .collect();
        return Err(format!(
            "auth misconfiguration: [server] requires all of jwks_url, issuer, audience together; missing: {missing:?}"
        )
        .into());
    }

    // Case C: vhost auth without server auth
    if present_count == 0 && any_vhost_has_auth {
        return Err(
            "auth misconfiguration: at least one [[vhost]] has login_url but \
            [server] is missing jwks_url/issuer/audience"
                .into(),
        );
    }

    // Case D: server auth without any vhost auth — allowed but useless; warn and proceed
    if !any_vhost_has_auth {
        log::warn!(
            "Server auth fields configured but no [[vhost]] has login_url; \
            AuthMiddleware will treat all requests as public"
        );
    }

    // At this point present_count == 3. If no vhost has auth (case D), we warned above
    // and still build the middleware (it will treat all requests as public).
    let jwks_url = config.server.jwks_url.as_deref().expect("checked above");
    let issuer = config
        .server
        .issuer
        .as_deref()
        .expect("checked above")
        .to_string();
    let audience = config
        .server
        .audience
        .as_deref()
        .expect("checked above")
        .to_string();

    info!("Fetching JWKS from {jwks_url} ...");
    let spki_der = crate::jwks::fetch_spki_der(jwks_url)
        .map_err(|e| format!("JWKS fetch failed at startup: {e}"))?;
    info!("JWKS fetched: {} byte SPKI", spki_der.len());

    // Build per-vhost configs from vhosts that opted in
    let mut vhost_configs = std::collections::HashMap::new();
    for vhost in &config.vhosts {
        if let Some(login_url) = vhost.login_url.as_deref() {
            vhost_configs.insert(
                vhost.domain.clone(),
                server::VhostAuthConfig {
                    login_url: login_url.to_string(),
                    public_paths: vhost.public_paths.clone(),
                },
            );
            info!(
                "Auth: vhost '{}' protected; {} public path(s); login -> {}",
                vhost.domain,
                vhost.public_paths.len(),
                login_url,
            );
        }
    }

    Ok(Some(server::AuthMiddleware::new(
        spki_der,
        vhost_configs,
        issuer,
        audience,
    )))
}

fn main() -> Result<()> {
    SimpleLogger::init()?;
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    let parsed_args = args::parse(&raw_args);
    let port = args::get_parsed::<u16>(&parsed_args, "--port", DEFAULT_PORT)?;
    let addr = format!("0.0.0.0:{}", port);
    let (dispatcher, plugin_configs, maybe_config) = build_dispatcher(&parsed_args)?;
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

    let auth_middleware = match maybe_config.as_ref() {
        Some(cfg) => build_auth_middleware(cfg)?,
        None => None, // --root mode has no auth
    };
    let middleware_chain = match auth_middleware {
        Some(mw) => MiddlewareChain::new().add(mw),
        None => MiddlewareChain::new(),
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
        let (_, _, maybe_config) = result.unwrap();
        assert!(
            maybe_config.is_none(),
            "--root mode should return None config"
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
        let (_, _, maybe_config) = result.unwrap();
        assert!(
            maybe_config.is_some(),
            "--config mode should return Some config"
        );
    }

    // ========== build_auth_middleware() unit tests ==========

    #[test]
    fn build_auth_middleware_no_auth_config_returns_none() {
        let cfg = config::Config {
            server: config::ServerConfig::default(),
            vhosts: vec![],
            plugins: vec![],
        };
        let result = build_auth_middleware(&cfg);
        assert!(matches!(result, Ok(None)), "no auth config -> Ok(None)");
    }

    #[test]
    fn build_auth_middleware_partial_server_config_returns_err() {
        let cfg = config::Config {
            server: config::ServerConfig {
                default_root: None,
                jwks_url: Some("https://x.auth0.com/jwks.json".to_string()),
                issuer: None,
                audience: None,
            },
            vhosts: vec![],
            plugins: vec![],
        };
        let result = build_auth_middleware(&cfg);
        assert!(result.is_err(), "partial server auth config -> Err");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("issuer"),
            "error must name missing field 'issuer': {msg}"
        );
        assert!(
            msg.contains("audience"),
            "error must name missing field 'audience': {msg}"
        );
    }

    #[test]
    fn build_auth_middleware_vhost_auth_without_server_auth_returns_err() {
        let cfg = config::Config {
            server: config::ServerConfig::default(),
            vhosts: vec![config::VhostEntry {
                domain: "ptodd.org".to_string(),
                root: "/var/www/ptodd.org".to_string(),
                login_url: Some("https://x.auth0.com/login".to_string()),
                public_paths: vec!["/".to_string()],
            }],
            plugins: vec![],
        };
        let result = build_auth_middleware(&cfg);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("login_url") && msg.contains("[server]"),
            "error must mention vhost login_url and [server]: {msg}"
        );
    }
}
