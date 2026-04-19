//! HTTP Request -- types re-exported from SDK, parse logic local.

pub use kiss_plugin_sdk::{Request, RequestMethod};

use super::*;

pub(super) const MAX_HEADER_LINES: usize = 100;

/// Parse raw HTTP request lines into a Request.
/// Server-internal: uses server::Error for detailed parse failure reporting.
pub fn parse_request(raw_request: &[String]) -> Result<Request> {
    if raw_request.len() > MAX_HEADER_LINES {
        return Err(Error::RequestTooLarge);
    }
    if raw_request.is_empty() {
        return Err(Error::InvalidRequest(String::from(
            "cannot parse empty request",
        )));
    }
    let control_data_parts: Vec<&str> = raw_request[0].split_ascii_whitespace().collect();
    if control_data_parts.len() != 3 {
        return Err(Error::InvalidRequest(format!(
            "control data: expected 3 parts, got {}",
            control_data_parts.len()
        )));
    }
    if control_data_parts[2] != "HTTP/1.1" {
        return Err(Error::InvalidRequest(format!(
            "unsupported HTTP version: expected 'HTTP/1.1', got '{}'",
            control_data_parts[2]
        )));
    }
    let host = raw_request[1..].iter().find_map(|line| {
        if line.len() >= 5 && line[..5].eq_ignore_ascii_case("host:") {
            Some(line[5..].trim().to_string())
        } else {
            None
        }
    });
    let headers: Vec<(String, String)> = raw_request[1..]
        .iter()
        .filter_map(|line| {
            let colon_pos = line.find(':')?;
            let name = line[..colon_pos].trim().to_string();
            let value = line[colon_pos + 1..].trim().to_string();
            Some((name, value))
        })
        .collect();
    let method = RequestMethod::try_from(control_data_parts[0])
        .map_err(|e| Error::InvalidRequest(e.to_string()))?;
    Ok(Request {
        method,
        target: kiss_plugin_sdk::Url::from(control_data_parts[1]),
        host,
        headers,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_get_root() {
        let lines = vec!["GET / HTTP/1.1".to_string()];
        assert!(parse_request(&lines).is_ok());
    }

    #[test]
    fn parse_too_many_headers() {
        // 101 lines should trigger RequestTooLarge
        let lines: Vec<String> = (0..=MAX_HEADER_LINES)
            .map(|i| format!("X-Header-{}: value", i))
            .collect();
        let result = parse_request(&lines);
        assert!(matches!(result, Err(Error::RequestTooLarge)));
    }

    #[test]
    fn parse_empty_returns_err() {
        let lines: Vec<String> = vec![];
        assert!(parse_request(&lines).is_err());
    }

    #[test]
    fn parse_host_header_present() {
        let lines = vec!["GET / HTTP/1.1".to_string(), "Host: ptodd.org".to_string()];
        let req = parse_request(&lines).unwrap();
        assert_eq!(req.host, Some("ptodd.org".to_string()));
    }

    #[test]
    fn parse_host_header_uppercase_raw_value() {
        let lines = vec!["GET / HTTP/1.1".to_string(), "HOST: PTODD.ORG".to_string()];
        let req = parse_request(&lines).unwrap();
        // Raw value stored as-is (normalization is a dispatch concern)
        assert_eq!(req.host, Some("PTODD.ORG".to_string()));
    }

    #[test]
    fn parse_host_header_absent() {
        let lines = vec!["GET / HTTP/1.1".to_string()];
        let req = parse_request(&lines).unwrap();
        assert_eq!(req.host, None);
    }

    #[test]
    fn parse_host_header_trimmed() {
        let lines = vec![
            "GET / HTTP/1.1".to_string(),
            "Host:  spaced.org  ".to_string(),
        ];
        let req = parse_request(&lines).unwrap();
        assert_eq!(req.host, Some("spaced.org".to_string()));
    }

    #[test]
    fn parse_single_request_line_host_is_none() {
        let lines = vec!["GET / HTTP/1.1".to_string()];
        let req = parse_request(&lines).unwrap();
        assert_eq!(req.host, None);
    }

    #[test]
    fn parse_collects_all_headers() {
        let lines = vec![
            "GET / HTTP/1.1".to_string(),
            "Host: example.com".to_string(),
            "X-Custom: value1".to_string(),
            "Accept: text/html".to_string(),
        ];
        let req = parse_request(&lines).unwrap();
        assert_eq!(req.headers.len(), 3, "should collect all 3 header lines");
    }

    #[test]
    fn header_accessor_case_insensitive() {
        let lines = vec![
            "GET / HTTP/1.1".to_string(),
            "X-Authenticated-User: alice".to_string(),
        ];
        let req = parse_request(&lines).unwrap();
        assert_eq!(req.header("x-authenticated-user"), Some("alice"));
        assert_eq!(req.header("X-AUTHENTICATED-USER"), Some("alice"));
        assert_eq!(req.header("X-Authenticated-User"), Some("alice"));
    }

    #[test]
    fn header_accessor_missing_header_returns_none() {
        let lines = vec!["GET / HTTP/1.1".to_string()];
        let req = parse_request(&lines).unwrap();
        assert_eq!(req.header("X-Missing"), None);
    }

    #[test]
    fn header_accessor_returns_first_match() {
        let lines = vec![
            "GET / HTTP/1.1".to_string(),
            "X-Dup: first".to_string(),
            "X-Dup: second".to_string(),
        ];
        let req = parse_request(&lines).unwrap();
        assert_eq!(req.header("X-Dup"), Some("first"));
    }
}
