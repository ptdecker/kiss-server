//! TOML configuration parser for virtual host mappings.

use std::fmt;

// ===== Types =====

#[derive(Debug, Clone)]
pub struct VhostEntry {
    pub domain: String,
    pub root: String,
}

#[derive(Debug, Clone, Default)]
pub struct ServerConfig {
    pub default_root: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub server: ServerConfig,
    pub vhosts: Vec<VhostEntry>,
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

/// Commits a VhostEntry, validating required fields.
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
    Ok(entry)
}

// ===== Config impl =====

impl Config {
    /// Parse a TOML config string into a Config.
    pub fn parse(input: &str) -> Result<Config, ConfigError> {
        let mut server = ServerConfig::default();
        let mut vhosts: Vec<VhostEntry> = Vec::new();
        let mut section = Section::None;
        let mut current_vhost: Option<VhostEntry> = None;

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
                current_vhost = Some(VhostEntry {
                    domain: String::new(),
                    root: String::new(),
                });
                section = Section::Vhost;
            } else if line == "[server]" {
                // Commit in-progress vhost if any
                if let Some(entry) = current_vhost.take() {
                    vhosts.push(commit_vhost(entry, lineno)?);
                }
                section = Section::Server;
            } else if line.starts_with('[') {
                return Err(ConfigError::Parse(format!(
                    "line {}: unrecognized section header: {}",
                    lineno + 1,
                    line
                )));
            } else {
                // Must be a key = "value" pair
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
                            ConfigError::Parse(format!("line {}: internal parse error", lineno + 1))
                        })?;
                        match key.as_str() {
                            "domain" => entry.domain = value,
                            "root" => entry.root = value,
                            _ => {
                                return Err(ConfigError::Parse(format!(
                                    "line {}: unknown key '{}' in [[vhost]] block",
                                    lineno + 1,
                                    key
                                )));
                            }
                        }
                    }
                }
            }
        }

        // Commit final in-progress vhost
        if let Some(entry) = current_vhost.take() {
            let last_lineno = input.lines().count();
            vhosts.push(commit_vhost(entry, last_lineno)?);
        }

        Ok(Config { server, vhosts })
    }

    /// Read and parse a config file from disk.
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
        // Just verify it's an error — message format is implementation-defined
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
}
