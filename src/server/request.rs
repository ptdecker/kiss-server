//! HTTP Request (v1.1)

use crate::url::Url;

use super::*;

/// Maximum number of header lines accepted in a single request.
/// Requests exceeding this limit are rejected with Error::RequestTooLarge.
pub(super) const MAX_HEADER_LINES: usize = 100;

/// Request Methods (RFC-9110 7.1)
///
/// Cf. <https://datatracker.ietf.org/doc/html/rfc9110#name-overview>
#[derive(Debug, PartialEq, Copy, Clone)]
pub enum RequestMethod {
    Get,
    Head,
    Post,
    Put,
    Delete,
    Connect,
    Options,
    Trace,
}

impl fmt::Display for RequestMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let method_str = match self {
            RequestMethod::Get => "GET",
            RequestMethod::Head => "HEAD",
            RequestMethod::Post => "POST",
            RequestMethod::Put => "PUT",
            RequestMethod::Delete => "DELETE",
            RequestMethod::Connect => "CONNECT",
            RequestMethod::Options => "OPTIONS",
            RequestMethod::Trace => "TRACE",
        };
        write!(f, "{}", method_str)
    }
}

impl TryFrom<&str> for RequestMethod {
    type Error = Error;
    fn try_from(value: &str) -> Result<Self> {
        match value {
            "GET" => Ok(RequestMethod::Get),
            "HEAD" => Ok(RequestMethod::Head),
            "POST" => Ok(RequestMethod::Post),
            "PUT" => Ok(RequestMethod::Put),
            "DELETE" => Ok(RequestMethod::Delete),
            "CONNECT" => Ok(RequestMethod::Connect),
            "OPTIONS" => Ok(RequestMethod::Options),
            "TRACE" => Ok(RequestMethod::Trace),
            _ => Err(Error::InvalidRequest(format!("invalid method: {value}"))),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Request {
    // Control Data (RFC-9110 6.2)
    //
    // The request method (RFC-9110 9)
    pub method: RequestMethod,
    // The request target (RFC-9110 7.1)
    pub target: Url,
}

impl Request {
    pub fn parse(raw_request: &[String]) -> Result<Request> {
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
        Ok(Request {
            method: control_data_parts[0].try_into()?,
            target: control_data_parts[1].into(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_get_root() {
        let lines = vec!["GET / HTTP/1.1".to_string()];
        assert!(Request::parse(&lines).is_ok());
    }

    #[test]
    fn parse_too_many_headers() {
        // 101 lines should trigger RequestTooLarge
        let lines: Vec<String> = (0..=MAX_HEADER_LINES)
            .map(|i| format!("X-Header-{}: value", i))
            .collect();
        let result = Request::parse(&lines);
        assert!(matches!(result, Err(Error::RequestTooLarge)));
    }

    #[test]
    fn parse_empty_returns_err() {
        let lines: Vec<String> = vec![];
        assert!(Request::parse(&lines).is_err());
    }
}
