//! CLI argument parsing for kiss-server.
//!
//! Parses a raw argv slice (excluding the binary name) into a map in a single
//! pass and exposes typed helpers for retrieving flags.

use std::collections::HashMap;
use std::path::PathBuf;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Parses a raw argv slice (excluding binary name) into a map of
/// flag -> optional value. Single pass, O(n). A flag is any token that
/// begins with `--`. The following token becomes its value unless that
/// token is itself another flag (begins with `--`), in which case the
/// first flag is recorded with `None` and the iterator continues at the
/// next flag. Non-flag tokens at positions where no flag is active are
/// silently ignored (positional args are not used by kiss-server).
pub fn parse(argv: &[String]) -> HashMap<String, Option<String>> {
    let mut map = HashMap::new();
    let mut i = 0;
    while i < argv.len() {
        let token = &argv[i];
        if token.starts_with("--") {
            // Look ahead for a value
            if i + 1 < argv.len() && !argv[i + 1].starts_with("--") {
                map.insert(token.clone(), Some(argv[i + 1].clone()));
                i += 2;
            } else {
                map.insert(token.clone(), None);
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    map
}

/// Returns `true` if `flag` is present in `map`, regardless of whether it
/// carries a value. Replaces `args.iter().any(|a| a == flag)`.
pub fn has_flag(map: &HashMap<String, Option<String>>, flag: &str) -> bool {
    map.contains_key(flag)
}

/// Validator kind for [`get_path`].
pub enum PathKind {
    /// The path must be an existing directory.
    Dir,
    /// The path must be an existing file.
    File,
}

/// Looks up `flag` in `map`; returns the validated [`PathBuf`] when the
/// flag is present with a value and the path exists as the requested
/// [`PathKind`].
///
/// Error messages mirror the current main.rs wording:
/// - absent: `"{flag} <path> is required"`
/// - missing value: `"{flag} requires a path argument"`
/// - wrong kind or missing:
///   - Dir: `"{flag} '{p}': not a directory or does not exist"`
///   - File: `"{flag} '{p}': not a file or does not exist"`
pub fn get_path(
    map: &HashMap<String, Option<String>>,
    flag: &str,
    kind: PathKind,
) -> Result<PathBuf> {
    match map.get(flag) {
        None => Err(format!("{} <path> is required", flag).into()),
        Some(None) => Err(format!("{} requires a path argument", flag).into()),
        Some(Some(path_str)) => {
            let path = PathBuf::from(path_str);
            match kind {
                PathKind::Dir => {
                    if !path.is_dir() {
                        return Err(format!(
                            "{} '{}': not a directory or does not exist",
                            flag, path_str
                        )
                        .into());
                    }
                }
                PathKind::File => {
                    if !path.is_file() {
                        return Err(format!(
                            "{} '{}': not a file or does not exist",
                            flag, path_str
                        )
                        .into());
                    }
                }
            }
            Ok(path)
        }
    }
}

/// Looks up `--port` in `map`; returns its parsed `u16` value, or
/// `default` if absent.
///
/// Error messages mirror current wording:
/// - missing value: `"--port requires a port number"`
/// - non-numeric: `"--port '{v}': not a valid port number"`
pub fn get_port(map: &HashMap<String, Option<String>>, default: u16) -> Result<u16> {
    match map.get("--port") {
        None => Ok(default),
        Some(None) => Err("--port requires a port number".into()),
        Some(Some(port_str)) => {
            let port: u16 = port_str
                .parse()
                .map_err(|_| format!("--port '{}': not a valid port number", port_str))?;
            Ok(port)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========== parse() unit tests ==========

    #[test]
    fn parse_empty_returns_empty_map() {
        let args: Vec<String> = vec![];
        let map = parse(&args);
        assert!(map.is_empty(), "expected empty map, got: {:?}", map);
    }

    #[test]
    fn parse_collects_all_three_flags_in_single_pass() {
        let args = vec![
            "--root".to_string(),
            "/tmp".to_string(),
            "--port".to_string(),
            "8080".to_string(),
            "--config".to_string(),
            "/x.toml".to_string(),
        ];
        let map = parse(&args);
        assert_eq!(map.get("--root"), Some(&Some("/tmp".to_string())));
        assert_eq!(map.get("--port"), Some(&Some("8080".to_string())));
        assert_eq!(map.get("--config"), Some(&Some("/x.toml".to_string())));
    }

    #[test]
    fn parse_bare_flag_followed_by_flag_yields_none_value() {
        let args = vec!["--skip".to_string(), "--port".to_string(), "80".to_string()];
        let map = parse(&args);
        assert_eq!(map.get("--skip"), Some(&None));
        assert_eq!(map.get("--port"), Some(&Some("80".to_string())));
    }

    #[test]
    fn parse_trailing_flag_no_value_records_none() {
        let args = vec!["--root".to_string()];
        let map = parse(&args);
        assert_eq!(map.get("--root"), Some(&None));
    }

    #[test]
    fn parse_single_flag_with_value() {
        let args = vec!["--port".to_string(), "8080".to_string()];
        let map = parse(&args);
        assert_eq!(map.get("--port"), Some(&Some("8080".to_string())));
    }

    // ========== has_flag() unit tests ==========

    #[test]
    fn has_flag_true_when_present_with_or_without_value() {
        let args_with_value = vec!["--config".to_string(), "/x.toml".to_string()];
        let map = parse(&args_with_value);
        assert!(
            has_flag(&map, "--config"),
            "expected true for flag with value"
        );

        let args_bare = vec!["--config".to_string()];
        let map_bare = parse(&args_bare);
        assert!(
            has_flag(&map_bare, "--config"),
            "expected true for bare flag"
        );
    }

    #[test]
    fn has_flag_false_when_absent() {
        let args: Vec<String> = vec![];
        let map = parse(&args);
        assert!(
            !has_flag(&map, "--config"),
            "expected false when flag absent"
        );
    }

    // ========== get_path() tests: --config ==========

    #[test]
    fn get_path_config_valid_file_returns_ok() {
        let dir = std::env::temp_dir();
        let file_path = dir.join("kiss_test_args_config_valid.toml");
        std::fs::write(&file_path, b"").unwrap();
        let path_str = file_path.to_string_lossy().to_string();
        let args = vec!["--config".to_string(), path_str];
        let map = parse(&args);
        let result = get_path(&map, "--config", PathKind::File);
        let _ = std::fs::remove_file(&file_path);
        assert!(
            result.is_ok(),
            "expected Ok for existing file, got: {:?}",
            result
        );
    }

    #[test]
    fn get_path_config_absent_returns_err() {
        let args: Vec<String> = vec![];
        let map = parse(&args);
        let result = get_path(&map, "--config", PathKind::File);
        assert!(result.is_err(), "expected Err when --config is absent");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("--config"),
            "error should mention --config, got: {:?}",
            msg
        );
    }

    #[test]
    fn get_path_config_missing_value_returns_err() {
        let args = vec!["--config".to_string()];
        let map = parse(&args);
        let result = get_path(&map, "--config", PathKind::File);
        assert!(
            result.is_err(),
            "expected Err when --config has no following path"
        );
    }

    #[test]
    fn get_path_config_nonexistent_returns_err() {
        let args = vec![
            "--config".to_string(),
            "/nonexistent/path/that/should/never/exist.toml".to_string(),
        ];
        let map = parse(&args);
        let result = get_path(&map, "--config", PathKind::File);
        assert!(result.is_err(), "expected Err for nonexistent path");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("not a file"),
            "error should say 'not a file', got: {:?}",
            msg
        );
    }

    #[test]
    fn get_path_config_directory_returns_err() {
        let dir = std::env::temp_dir().to_string_lossy().to_string();
        let args = vec!["--config".to_string(), dir];
        let map = parse(&args);
        let result = get_path(&map, "--config", PathKind::File);
        assert!(result.is_err(), "expected Err when path is a directory");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("not a file"),
            "error should say 'not a file', got: {:?}",
            msg
        );
    }

    // ========== get_path() tests: --root ==========

    #[test]
    fn get_path_root_valid_dir_returns_ok() {
        let temp = std::env::temp_dir().to_string_lossy().to_string();
        let args = vec!["--root".to_string(), temp];
        let map = parse(&args);
        let result = get_path(&map, "--root", PathKind::Dir);
        assert!(
            result.is_ok(),
            "expected Ok for existing directory, got: {:?}",
            result
        );
    }

    #[test]
    fn get_path_root_absent_returns_err() {
        let args: Vec<String> = vec![];
        let map = parse(&args);
        let result = get_path(&map, "--root", PathKind::Dir);
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
    fn get_path_root_missing_value_returns_err() {
        let args = vec!["--root".to_string()];
        let map = parse(&args);
        let result = get_path(&map, "--root", PathKind::Dir);
        assert!(
            result.is_err(),
            "expected Err when --root has no following path"
        );
    }

    #[test]
    fn get_path_root_nonexistent_returns_err() {
        let args = vec![
            "--root".to_string(),
            "/nonexistent/path/that/should/never/exist".to_string(),
        ];
        let map = parse(&args);
        let result = get_path(&map, "--root", PathKind::Dir);
        assert!(result.is_err(), "expected Err for nonexistent path");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("--root"),
            "error message should mention --root, got: {:?}",
            msg
        );
    }

    // ========== get_port() unit tests ==========

    #[test]
    fn get_port_valid_returns_ok() {
        let args = vec!["--port".to_string(), "8080".to_string()];
        let map = parse(&args);
        let result = get_port(&map, 6502);
        assert!(
            result.is_ok(),
            "expected Ok(8080) for valid port, got: {:?}",
            result
        );
        assert_eq!(result.unwrap(), 8080u16);
    }

    #[test]
    fn get_port_absent_returns_default() {
        let args: Vec<String> = vec![];
        let map = parse(&args);
        let result = get_port(&map, 6502);
        assert!(
            result.is_ok(),
            "expected Ok(6502) when --port is absent, got: {:?}",
            result
        );
        assert_eq!(result.unwrap(), 6502u16);
    }

    #[test]
    fn get_port_invalid_returns_err() {
        let args = vec!["--port".to_string(), "abc".to_string()];
        let map = parse(&args);
        let result = get_port(&map, 6502);
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
