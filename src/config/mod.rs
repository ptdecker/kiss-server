//! TOML configuration parser for virtual host mappings.

use std::fmt;

// ===== Types =====

#[derive(Debug, Clone)]
pub struct VhostEntry {
    pub domain: String,
    pub root: String,
    pub login_url: Option<String>, // ACFG-01
    pub public_paths: Vec<String>, // ACFG-02; empty Vec = no exemptions
}

#[derive(Debug, Clone)]
pub struct PluginConfig {
    pub name: String,
    pub extra: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Default)]
pub struct ServerConfig {
    pub default_root: Option<String>,
    pub jwks_url: Option<String>, // CRYP-05 (used in Plan 05)
    pub issuer: Option<String>,   // JWT iss validation (used in Plan 04)
    pub audience: Option<String>, // JWT aud validation (used in Plan 04)
}

#[derive(Debug, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub vhosts: Vec<VhostEntry>,
    pub plugins: Vec<PluginConfig>,
}

// ===== Error =====

#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    Parse(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Io(e) => write!(f, "config io error: {e}"),
            ConfigError::Parse(msg) => write!(f, "config parse error: {msg}"),
        }
    }
}

impl std::error::Error for ConfigError {}

impl From<std::io::Error> for ConfigError {
    fn from(e: std::io::Error) -> Self {
        ConfigError::Io(e)
    }
}

// ===== Parser internals =====

#[derive(Debug, PartialEq)]
enum Section {
    None,
    Server,
    Vhost,
    Plugin,
}

/// Parses a TOML inline array value `["a", "b", "c"]` into Vec<String>.
/// Only single-line arrays are supported. Each element must be a quoted string.
fn parse_inline_array(value_raw: &str, lineno: usize) -> Result<Vec<String>, ConfigError> {
    if !value_raw.starts_with('[') || !value_raw.ends_with(']') {
        return Err(ConfigError::Parse(format!(
            "line {}: inline array must start with '[' and end with ']', got: {}",
            lineno + 1,
            value_raw
        )));
    }
    let inner = value_raw[1..value_raw.len() - 1].trim();
    if inner.is_empty() {
        return Ok(Vec::new());
    }
    let mut result = Vec::new();
    for element in inner.split(',') {
        let el = element.trim();
        if el.len() < 2 || !el.starts_with('"') || !el.ends_with('"') {
            return Err(ConfigError::Parse(format!(
                "line {}: array element must be a non-empty quoted string, got: {}",
                lineno + 1,
                el
            )));
        }
        result.push(el[1..el.len() - 1].to_string());
    }
    Ok(result)
}

/// Parses a `key = "value"` line. Returns `(key, value)` with quotes stripped.
/// Returns `Err` if the value is not a quoted string or the format is wrong.
fn parse_key_value(line: &str, lineno: usize) -> Result<(String, String), ConfigError> {
    let eq_pos = line.find('=').ok_or_else(|| {
        ConfigError::Parse(format!(
            "line {}: expected 'key = \"value\"', got: {}",
            lineno + 1,
            line
        ))
    })?;
    let key = line[..eq_pos].trim().to_string();
    let value_raw = line[eq_pos + 1..].trim();

    if !value_raw.starts_with('"') || !value_raw.ends_with('"') || value_raw.len() < 2 {
        return Err(ConfigError::Parse(format!(
            "line {}: value must be a quoted string (double quotes), got: {}",
            lineno + 1,
            value_raw
        )));
    }
    // Strip surrounding double quotes
    let value = value_raw[1..value_raw.len() - 1].to_string();
    Ok((key, value))
}

/// Commits a VhostEntry, validating required fields and auth-config consistency.
fn commit_vhost(entry: VhostEntry, lineno: usize) -> Result<VhostEntry, ConfigError> {
    if entry.domain.is_empty() {
        return Err(ConfigError::Parse(format!(
            "line {}: [[vhost]] field 'domain' must not be empty",
            lineno + 1
        )));
    }
    if entry.root.is_empty() {
        return Err(ConfigError::Parse(format!(
            "line {}: [[vhost]] field 'root' must not be empty",
            lineno + 1
        )));
    }
    // Phase 25 ACFG-04: login_url and public_paths must be set together
    if entry.login_url.is_some() && entry.public_paths.is_empty() {
        return Err(ConfigError::Parse(format!(
            "line {}: [[vhost]] '{}' has 'login_url' but no 'public_paths' \
            (set public_paths = [] explicitly only if you want every path protected — \
            currently this is treated as a misconfiguration)",
            lineno + 1,
            entry.domain
        )));
    }
    if entry.login_url.is_none() && !entry.public_paths.is_empty() {
        return Err(ConfigError::Parse(format!(
            "line {}: [[vhost]] '{}' has 'public_paths' but no 'login_url' \
            (cannot redirect unauthenticated requests without a login_url)",
            lineno + 1,
            entry.domain
        )));
    }
    Ok(entry)
}

/// Commits a PluginConfig, validating required fields.
fn commit_plugin(entry: PluginConfig, lineno: usize) -> Result<PluginConfig, ConfigError> {
    if entry.name.is_empty() {
        return Err(ConfigError::Parse(format!(
            "line {}: [[plugin]] field 'name' must not be empty",
            lineno + 1
        )));
    }
    Ok(entry)
}

// ===== Config impl =====

impl Config {
    /// Parse a TOML config string into a Config.
    pub fn parse(input: &str) -> Result<Config, ConfigError> {
        let mut server = ServerConfig::default();
        let mut vhosts: Vec<VhostEntry> = Vec::new();
        let mut plugins: Vec<PluginConfig> = Vec::new();
        let mut section = Section::None;
        let mut current_vhost: Option<VhostEntry> = None;
        let mut current_plugin: Option<PluginConfig> = None;

        for (lineno, raw_line) in input.lines().enumerate() {
            let line = raw_line.trim();

            // Skip blank lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if line == "[[vhost]]" {
                // Commit in-progress vhost if any
                if let Some(entry) = current_vhost.take() {
                    vhosts.push(commit_vhost(entry, lineno)?);
                }
                if let Some(entry) = current_plugin.take() {
                    plugins.push(commit_plugin(entry, lineno)?);
                }
                current_vhost = Some(VhostEntry {
                    domain: String::new(),
                    root: String::new(),
                    login_url: None,
                    public_paths: Vec::new(),
                });
                section = Section::Vhost;
            } else if line == "[[plugin]]" {
                // Commit any in-progress entries before switching a section
                if let Some(entry) = current_vhost.take() {
                    vhosts.push(commit_vhost(entry, lineno)?);
                }
                if let Some(entry) = current_plugin.take() {
                    plugins.push(commit_plugin(entry, lineno)?);
                }
                current_plugin = Some(PluginConfig {
                    name: String::new(),
                    extra: std::collections::HashMap::new(),
                });
                section = Section::Plugin;
            } else if line == "[server]" {
                // Commit in-progress vhost if any
                if let Some(entry) = current_vhost.take() {
                    vhosts.push(commit_vhost(entry, lineno)?);
                }
                if let Some(entry) = current_plugin.take() {
                    plugins.push(commit_plugin(entry, lineno)?);
                }
                section = Section::Server;
            } else if line.starts_with('[') {
                return Err(ConfigError::Parse(format!(
                    "line {}: unrecognized section header: {}",
                    lineno + 1,
                    line
                )));
            } else {
                // Split key and value at '='
                let eq_pos = line.find('=').ok_or_else(|| {
                    ConfigError::Parse(format!(
                        "line {}: expected 'key = value', got: {}",
                        lineno + 1,
                        line
                    ))
                })?;
                let key = line[..eq_pos].trim().to_string();
                let value_raw = line[eq_pos + 1..].trim();

                if value_raw.starts_with('[') {
                    // Inline array branch — only [[vhost]] public_paths is allowed
                    match (&section, key.as_str()) {
                        (Section::Vhost, "public_paths") => {
                            let entry = current_vhost.as_mut().ok_or_else(|| {
                                ConfigError::Parse(format!(
                                    "line {}: internal parse error",
                                    lineno + 1
                                ))
                            })?;
                            entry.public_paths = parse_inline_array(value_raw, lineno)?;
                        }
                        _ => {
                            return Err(ConfigError::Parse(format!(
                                "line {}: unexpected array value for key '{}' \
                                (only [[vhost]] public_paths supports arrays)",
                                lineno + 1,
                                key
                            )));
                        }
                    }
                } else {
                    // String value branch — use parse_key_value for quoted-string validation
                    let (key, value) = parse_key_value(line, lineno)?;
                    match section {
                        Section::None => {
                            return Err(ConfigError::Parse(format!(
                                "line {}: key '{}' appears outside any section",
                                lineno + 1,
                                key
                            )));
                        }
                        Section::Server => match key.as_str() {
                            "default_root" => server.default_root = Some(value),
                            "jwks_url" => server.jwks_url = Some(value), // Phase 25
                            "issuer" => server.issuer = Some(value),     // Phase 25
                            "audience" => server.audience = Some(value), // Phase 25
                            _ => {
                                return Err(ConfigError::Parse(format!(
                                    "line {}: unknown key '{}' in [server] section",
                                    lineno + 1,
                                    key
                                )));
                            }
                        },
                        Section::Vhost => {
                            let entry = current_vhost.as_mut().ok_or_else(|| {
                                ConfigError::Parse(format!(
                                    "line {}: internal parse error",
                                    lineno + 1
                                ))
                            })?;
                            match key.as_str() {
                                "domain" => entry.domain = value,
                                "root" => entry.root = value,
                                "login_url" => entry.login_url = Some(value), // Phase 25 ACFG-01
                                _ => {
                                    return Err(ConfigError::Parse(format!(
                                        "line {}: unknown key '{}' in [[vhost]] block",
                                        lineno + 1,
                                        key
                                    )));
                                }
                            }
                        }
                        Section::Plugin => {
                            let entry = current_plugin.as_mut().ok_or_else(|| {
                                ConfigError::Parse(format!(
                                    "line {}: internal parse error",
                                    lineno + 1
                                ))
                            })?;
                            match key.as_str() {
                                "name" => entry.name = value,
                                other => {
                                    entry.extra.insert(other.to_string(), value);
                                }
                            }
                        }
                    }
                }
            }
        }

        // Commit final in-progress vhost
        if let Some(entry) = current_vhost.take() {
            let last_lineno = input.lines().count().saturating_sub(1);
            vhosts.push(commit_vhost(entry, last_lineno)?);
        }

        // Commit the final in-progress plugin
        if let Some(entry) = current_plugin.take() {
            let last_lineno = input.lines().count().saturating_sub(1);
            plugins.push(commit_plugin(entry, last_lineno)?);
        }

        Ok(Config {
            server,
            vhosts,
            plugins,
        })
    }

    /// Read and parse a config file from the disk.
    pub fn load(path: &std::path::Path) -> Result<Config, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        Config::parse(&content)
    }
}

// ===== Tests =====

#[cfg(test)]
mod tests {
    use super::*;

    // ===== Valid config tests =====

    #[test]
    fn parse_basic_single_vhost() {
        let input = r#"
[[vhost]]
domain = "ptodd.org"
root = "/var/www/ptodd.org"
"#;
        let cfg = Config::parse(input).expect("should parse");
        assert_eq!(cfg.vhosts.len(), 1);
        assert_eq!(cfg.vhosts[0].domain, "ptodd.org");
        assert_eq!(cfg.vhosts[0].root, "/var/www/ptodd.org");
    }

    #[test]
    fn parse_two_vhost_entries() {
        let input = r#"
[[vhost]]
domain = "ptodd.org"
root = "/var/www/ptodd.org"

[[vhost]]
domain = "example.org"
root = "/var/www/example.org"
"#;
        let cfg = Config::parse(input).expect("should parse");
        assert_eq!(cfg.vhosts.len(), 2);
    }

    #[test]
    fn parse_server_section_with_default_root() {
        let input = r#"
[server]
default_root = "/var/www/default"

[[vhost]]
domain = "ptodd.org"
root = "/var/www/ptodd.org"
"#;
        let cfg = Config::parse(input).expect("should parse");
        assert_eq!(
            cfg.server.default_root,
            Some("/var/www/default".to_string())
        );
        assert_eq!(cfg.vhosts.len(), 1);
    }

    #[test]
    fn parse_server_after_vhost_blocks() {
        let input = r#"
[[vhost]]
domain = "ptodd.org"
root = "/var/www/ptodd.org"

[server]
default_root = "/var/www/default"
"#;
        let cfg = Config::parse(input).expect("should parse with [server] after [[vhost]]");
        assert_eq!(cfg.vhosts.len(), 1);
        assert_eq!(cfg.vhosts[0].domain, "ptodd.org");
        assert_eq!(
            cfg.server.default_root,
            Some("/var/www/default".to_string())
        );
    }

    #[test]
    fn parse_server_before_vhost_blocks() {
        let input = r#"
[server]
default_root = "/var/www/default"

[[vhost]]
domain = "ptodd.org"
root = "/var/www/ptodd.org"
"#;
        let cfg = Config::parse(input).expect("should parse with [server] before [[vhost]]");
        assert_eq!(cfg.vhosts.len(), 1);
        assert_eq!(
            cfg.server.default_root,
            Some("/var/www/default".to_string())
        );
    }

    #[test]
    fn parse_comments_and_blank_lines_skipped() {
        let input = r#"
# This is a comment
# Another comment

[[vhost]]
# comment inside vhost block
domain = "ptodd.org"

root = "/var/www/ptodd.org"
# trailing comment
"#;
        let cfg = Config::parse(input).expect("comments and blank lines should be skipped");
        assert_eq!(cfg.vhosts.len(), 1);
        assert_eq!(cfg.vhosts[0].domain, "ptodd.org");
    }

    #[test]
    fn parse_no_server_section_gives_none_default_root() {
        let input = r#"
[[vhost]]
domain = "ptodd.org"
root = "/var/www/ptodd.org"
"#;
        let cfg = Config::parse(input).expect("should parse");
        assert_eq!(cfg.server.default_root, None);
    }

    #[test]
    fn parse_empty_config_gives_empty_vhosts_and_no_default_root() {
        let cfg = Config::parse("").expect("empty config should parse as Ok");
        assert_eq!(cfg.vhosts.len(), 0);
        assert_eq!(cfg.server.default_root, None);
    }

    // ===== Error cases =====

    #[test]
    fn parse_missing_domain_field_returns_err_with_domain_in_message() {
        let input = r#"
[[vhost]]
root = "/var/www/ptodd.org"
"#;
        let err = Config::parse(input).expect_err("missing domain should be an error");
        let msg = err.to_string();
        assert!(
            msg.contains("domain"),
            "error message should mention 'domain', got: {:?}",
            msg
        );
    }

    #[test]
    fn parse_missing_root_field_returns_err_with_root_in_message() {
        let input = r#"
[[vhost]]
domain = "ptodd.org"
"#;
        let err = Config::parse(input).expect_err("missing root should be an error");
        let msg = err.to_string();
        assert!(
            msg.contains("root"),
            "error message should mention 'root', got: {:?}",
            msg
        );
    }

    #[test]
    fn parse_key_outside_section_returns_err_with_outside_in_message() {
        let input = r#"domain = "ptodd.org""#;
        let err = Config::parse(input).expect_err("key outside section should be an error");
        let msg = err.to_string();
        assert!(
            msg.contains("outside"),
            "error message should mention 'outside', got: {:?}",
            msg
        );
    }

    #[test]
    fn parse_unquoted_value_returns_err() {
        let input = r#"
[[vhost]]
domain = ptodd.org
root = "/var/www/ptodd.org"
"#;
        let err = Config::parse(input).expect_err("unquoted value should be an error");
        let _msg = err.to_string();
        // Just verify it's an error — a message format is implementation-defined
    }

    #[test]
    fn parse_unrecognized_section_header_returns_err() {
        let input = r#"
[unknown]
foo = "bar"
"#;
        let err = Config::parse(input).expect_err("unknown section should be an error");
        let _msg = err.to_string();
    }

    // ===== Config::load tests =====

    #[test]
    fn load_from_nonexistent_file_returns_err() {
        let result = Config::load(std::path::Path::new("/nonexistent/path/config.toml"));
        assert!(
            result.is_err(),
            "load from nonexistent file should return Err"
        );
    }

    #[test]
    fn load_from_valid_file_returns_ok() {
        let tmp_dir = std::env::temp_dir();
        let tmp_path = tmp_dir.join("kiss_server_config_test.toml");
        std::fs::write(
            &tmp_path,
            r#"
[[vhost]]
domain = "ptodd.org"
root = "/var/www/ptodd.org"
"#,
        )
        .expect("failed to write temp file");

        let result = Config::load(&tmp_path);
        // Clean up regardless of outcome
        let _ = std::fs::remove_file(&tmp_path);
        assert!(
            result.is_ok(),
            "load from valid file should return Ok: {:?}",
            result
        );
        let cfg = result.unwrap();
        assert_eq!(cfg.vhosts.len(), 1);
        assert_eq!(cfg.vhosts[0].domain, "ptodd.org");
    }

    // ===== [[plugin]] config tests (Phase 21: PLUG-03, D-05, D-06, D-07, D-08) =====

    #[test]
    fn parse_single_plugin_with_name() {
        let input = r#"
[[plugin]]
name = "url-shortener"
"#;
        let cfg = Config::parse(input).expect("should parse plugin");
        assert_eq!(cfg.plugins.len(), 1, "expected 1 plugin");
        assert_eq!(cfg.plugins[0].name, "url-shortener");
        assert!(cfg.plugins[0].extra.is_empty(), "no extra keys expected");
    }

    #[test]
    fn parse_plugin_with_extra_keys() {
        let input = r#"
[[plugin]]
name = "url-shortener"
base_url = "https://ptodd.org"
max_entries = "10000"
"#;
        let cfg = Config::parse(input).expect("should parse plugin with extras");
        assert_eq!(cfg.plugins.len(), 1);
        assert_eq!(cfg.plugins[0].name, "url-shortener");
        assert_eq!(
            cfg.plugins[0].extra.get("base_url"),
            Some(&"https://ptodd.org".to_string()),
            "base_url should be in extra"
        );
        assert_eq!(
            cfg.plugins[0].extra.get("max_entries"),
            Some(&"10000".to_string()),
            "max_entries should be in extra"
        );
    }

    #[test]
    fn parse_two_plugins() {
        let input = r#"
[[plugin]]
name = "url-shortener"

[[plugin]]
name = "blog"
"#;
        let cfg = Config::parse(input).expect("should parse two plugins");
        assert_eq!(cfg.plugins.len(), 2, "expected 2 plugins");
        assert_eq!(cfg.plugins[0].name, "url-shortener");
        assert_eq!(cfg.plugins[1].name, "blog");
    }

    #[test]
    fn parse_plugin_missing_name_returns_err() {
        let input = r#"
[[plugin]]
base_url = "https://ptodd.org"
"#;
        let err = Config::parse(input).expect_err("missing name should be error");
        let msg = err.to_string();
        assert!(
            msg.contains("name"),
            "error should mention 'name', got: {:?}",
            msg
        );
    }

    #[test]
    fn parse_vhost_then_plugin() {
        let input = r#"
[[vhost]]
domain = "ptodd.org"
root = "/var/www/ptodd.org"

[[plugin]]
name = "url-shortener"
"#;
        let cfg = Config::parse(input).expect("should parse vhost then plugin");
        assert_eq!(cfg.vhosts.len(), 1, "expected 1 vhost");
        assert_eq!(cfg.plugins.len(), 1, "expected 1 plugin");
    }

    #[test]
    fn parse_plugin_then_vhost() {
        let input = r#"
[[plugin]]
name = "url-shortener"

[[vhost]]
domain = "ptodd.org"
root = "/var/www/ptodd.org"
"#;
        let cfg = Config::parse(input).expect("should parse plugin then vhost");
        assert_eq!(cfg.vhosts.len(), 1, "expected 1 vhost");
        assert_eq!(cfg.plugins.len(), 1, "expected 1 plugin");
    }

    #[test]
    fn parse_plugin_between_vhosts() {
        let input = r#"
[[vhost]]
domain = "ptodd.org"
root = "/var/www/ptodd.org"

[[plugin]]
name = "url-shortener"

[[vhost]]
domain = "example.org"
root = "/var/www/example.org"
"#;
        let cfg = Config::parse(input).expect("should parse plugin between vhosts");
        assert_eq!(cfg.vhosts.len(), 2, "expected 2 vhosts");
        assert_eq!(cfg.plugins.len(), 1, "expected 1 plugin");
    }

    #[test]
    fn parse_no_plugins_gives_empty_vec() {
        let input = r#"
[[vhost]]
domain = "ptodd.org"
root = "/var/www/ptodd.org"
"#;
        let cfg = Config::parse(input).expect("should parse");
        assert!(
            cfg.plugins.is_empty(),
            "no [[plugin]] blocks means empty vec"
        );
    }

    #[test]
    fn parse_empty_config_gives_empty_plugins() {
        let cfg = Config::parse("").expect("empty should parse");
        assert!(cfg.plugins.is_empty());
    }

    #[test]
    fn parse_server_then_plugin() {
        let input = r#"
[server]
default_root = "/var/www/default"

[[plugin]]
name = "url-shortener"
"#;
        let cfg = Config::parse(input).expect("should parse server then plugin");
        assert_eq!(cfg.plugins.len(), 1);
        assert_eq!(
            cfg.server.default_root,
            Some("/var/www/default".to_string())
        );
    }

    // ===== parse_inline_array tests (Task 1, Phase 25 ACFG-02) =====

    #[test]
    fn parse_inline_array_empty() {
        let result = parse_inline_array("[]", 0);
        assert!(
            result.is_ok(),
            "empty array should parse, got: {:?}",
            result
        );
        assert_eq!(result.unwrap(), Vec::<String>::new());
    }

    #[test]
    fn parse_inline_array_single_element() {
        let result = parse_inline_array("[\"/\"]", 0);
        assert!(
            result.is_ok(),
            "single-element array should parse, got: {:?}",
            result
        );
        assert_eq!(result.unwrap(), vec!["/".to_string()]);
    }

    #[test]
    fn parse_inline_array_three_elements() {
        let result = parse_inline_array("[\"/\", \"/about\", \"/static\"]", 0);
        assert!(
            result.is_ok(),
            "three-element array should parse, got: {:?}",
            result
        );
        assert_eq!(result.unwrap(), vec!["/", "/about", "/static"]);
    }

    #[test]
    fn parse_inline_array_unclosed() {
        let result = parse_inline_array("[", 0);
        assert!(result.is_err(), "unclosed array should be an error");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("inline array"),
            "error should mention 'inline array', got: {:?}",
            msg
        );
    }

    #[test]
    fn parse_inline_array_unquoted_element() {
        let result = parse_inline_array("[foo]", 0);
        assert!(result.is_err(), "unquoted element should be an error");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("quoted string"),
            "error should mention 'quoted string', got: {:?}",
            msg
        );
    }

    #[test]
    fn parse_inline_array_trailing_comma() {
        let result = parse_inline_array("[\"/\",]", 0);
        assert!(result.is_err(), "trailing comma should be an error");
    }

    // ===== Task 2 tests: dispatch wiring and commit_vhost auth validation =====

    #[test]
    fn parse_vhost_login_url() {
        let input = r#"
[[vhost]]
domain = "ptodd.org"
root = "/var/www/ptodd.org"
login_url = "https://ptodd.auth0.com/login"
public_paths = ["/"]
"#;
        let cfg = Config::parse(input).expect("should parse vhost with login_url and public_paths");
        assert_eq!(
            cfg.vhosts[0].login_url.as_deref(),
            Some("https://ptodd.auth0.com/login")
        );
        assert_eq!(cfg.vhosts[0].public_paths, vec!["/"]);
    }

    #[test]
    fn parse_vhost_public_paths_array() {
        let input = r#"
[[vhost]]
domain = "ptodd.org"
root = "/var/www/ptodd.org"
login_url = "https://ptodd.auth0.com/login"
public_paths = ["/", "/about", "/favicon.ico"]
"#;
        let cfg = Config::parse(input).expect("should parse vhost with public_paths array");
        assert_eq!(cfg.vhosts[0].public_paths.len(), 3);
    }

    #[test]
    fn parse_vhost_no_auth_fields_is_ok() {
        let input = r#"
[[vhost]]
domain = "ptodd.org"
root = "/var/www/ptodd.org"
"#;
        let cfg = Config::parse(input).expect("vhost with no auth fields should parse ok");
        assert_eq!(cfg.vhosts[0].login_url, None);
        assert!(cfg.vhosts[0].public_paths.is_empty());
    }

    #[test]
    fn parse_vhost_login_url_without_public_paths_errors() {
        let input = r#"
[[vhost]]
domain = "ptodd.org"
root = "/var/www/ptodd.org"
login_url = "https://ptodd.auth0.com/login"
"#;
        let err = Config::parse(input).expect_err("login_url without public_paths should error");
        let msg = err.to_string();
        assert!(
            msg.contains("ptodd.org"),
            "error should contain domain name, got: {:?}",
            msg
        );
    }

    #[test]
    fn parse_vhost_public_paths_without_login_url_errors() {
        let input = r#"
[[vhost]]
domain = "ptodd.org"
root = "/var/www/ptodd.org"
public_paths = ["/"]
"#;
        let err = Config::parse(input).expect_err("public_paths without login_url should error");
        let msg = err.to_string();
        assert!(
            msg.contains("ptodd.org"),
            "error should contain domain name, got: {:?}",
            msg
        );
    }

    #[test]
    fn parse_server_jwks_url() {
        let input = r#"
[server]
jwks_url = "https://x.auth0.com/.well-known/jwks.json"

[[vhost]]
domain = "ptodd.org"
root = "/var/www/ptodd.org"
"#;
        let cfg = Config::parse(input).expect("should parse server with jwks_url");
        assert_eq!(
            cfg.server.jwks_url.as_deref(),
            Some("https://x.auth0.com/.well-known/jwks.json")
        );
    }

    #[test]
    fn parse_server_issuer_and_audience() {
        let input = r#"
[server]
issuer = "https://x.auth0.com/"
audience = "abc123"

[[vhost]]
domain = "ptodd.org"
root = "/var/www/ptodd.org"
"#;
        let cfg = Config::parse(input).expect("should parse server with issuer and audience");
        assert_eq!(cfg.server.issuer.as_deref(), Some("https://x.auth0.com/"));
        assert_eq!(cfg.server.audience.as_deref(), Some("abc123"));
    }

    #[test]
    fn parse_unknown_server_key_still_errors() {
        let input = r#"
[server]
weird_key = "x"
"#;
        let err = Config::parse(input).expect_err("unknown server key should error");
        let msg = err.to_string();
        assert!(
            msg.contains("unknown key"),
            "error should say 'unknown key', got: {:?}",
            msg
        );
        assert!(
            msg.contains("server") || msg.contains("[server]"),
            "error should mention server section, got: {:?}",
            msg
        );
    }

    #[test]
    fn parse_unknown_vhost_key_still_errors() {
        let input = r#"
[[vhost]]
domain = "ptodd.org"
root = "/var/www/ptodd.org"
weird = "x"
"#;
        let err = Config::parse(input).expect_err("unknown vhost key should error");
        let msg = err.to_string();
        assert!(
            msg.contains("unknown key"),
            "error should say 'unknown key', got: {:?}",
            msg
        );
        assert!(
            msg.contains("vhost") || msg.contains("[[vhost]]"),
            "error should mention vhost section, got: {:?}",
            msg
        );
    }

    #[test]
    fn parse_array_for_non_public_paths_key_errors() {
        let input = r#"
[[vhost]]
domain = ["foo"]
root = "/var/www/ptodd.org"
"#;
        let err =
            Config::parse(input).expect_err("array value for non-public_paths key should error");
        let msg = err.to_string();
        assert!(
            msg.contains("unexpected array value"),
            "error should mention 'unexpected array value', got: {:?}",
            msg
        );
    }

    #[test]
    fn parse_public_paths_unquoted_element_errors() {
        let input = r#"
[[vhost]]
domain = "ptodd.org"
root = "/var/www/ptodd.org"
login_url = "https://ptodd.auth0.com/login"
public_paths = [foo]
"#;
        let err = Config::parse(input).expect_err("unquoted element in public_paths should error");
        let msg = err.to_string();
        assert!(
            msg.contains("quoted string"),
            "error should mention 'quoted string', got: {:?}",
            msg
        );
    }
}
