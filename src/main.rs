//! Provides the backend implementation for the ptodd.org website.

use log::{debug, info, warn};

use logger::SimpleLogger;
use server::{Router, Server};

use handlers::{RootHandler, StaticFileHandler};

mod handlers;
mod logger;
mod server;
mod time;
mod url;

const DEFAULT_ADDR: &str = "localhost:6502";

pub type Error = Box<dyn std::error::Error>;
pub type Result<T> = std::result::Result<T, Error>;

/// Parses `--root <path>` from the provided args slice (excluding the binary name at index 0).
///
/// Factored out of `parse_root()` so it can be unit-tested without `std::env::args()`.
fn parse_root_from(args: &[String]) -> crate::Result<std::path::PathBuf> {
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

/// Parses `--root <path>` from `std::env::args()`.
///
/// Returns the validated path (confirms it is a directory before returning).
/// `StaticFileHandler::new()` will canonicalize it further.
fn parse_root() -> crate::Result<std::path::PathBuf> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    parse_root_from(&args)
}

fn main() -> Result<()> {
    SimpleLogger::init()?;
    let root = parse_root()?;
    info!("Serving static files from root: {}", root.display());
    let handler = StaticFileHandler::new(root)?;
    let mut router = Router::new();
    router.add("GET", "/", RootHandler)?;
    let router = router.set_fallback(handler);
    Server::new(DEFAULT_ADDR)?.with_router(router).run()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
