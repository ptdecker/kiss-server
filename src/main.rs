//! A from-scratch HTTP/1.1 static file server written in pure Rust.
//! Runs behind CloudFront for TLS termination at ptodd.org / www.ptodd.org.

use log::info;

use logger::SimpleLogger;
use server::{Router, Server};

use handlers::StaticFileHandler;

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

/// Parses `--port <num>` from the provided args slice (excluding the binary name at index 0).
///
/// Returns the port number as `u16`, or `DEFAULT_PORT` if `--port` is absent.
fn parse_port_from(args: &[String]) -> crate::Result<u16> {
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

fn main() -> Result<()> {
    SimpleLogger::init()?;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = parse_root_from(&args)?;
    let port = parse_port_from(&args)?;
    let addr = format!("0.0.0.0:{}", port);
    info!("Serving static files from root: {}", root.display());
    let handler = StaticFileHandler::new(root)?;
    let router = Router::new().set_fallback(handler);
    Server::new(&addr)?.with_router(router).run()?;
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
