//! HTTP Response (v1.1)

use std::io::Write;

/// An HTTP/1.1 response, constructed via a value-chaining builder.
///
/// Build order: Response::new(status, reason).header(k, v)...body(bytes)
/// Send order: response.write_to(&mut stream)?
pub struct Response {
    status: u16,
    reason: &'static str,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Response {
    /// Construct a new response with the given status code and reason phrase.
    pub fn new(status: u16, reason: &'static str) -> Self {
        Response {
            status,
            reason,
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    /// Add a header to the response (value-chaining builder).
    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_string(), value.to_string()));
        self
    }

    /// Set the body of the response (value-chaining builder).
    pub fn body(mut self, body: Vec<u8>) -> Self {
        self.body = body;
        self
    }

    /// Returns the HTTP status code.
    pub fn status(&self) -> u16 {
        self.status
    }

    /// Returns the body length in bytes.
    pub fn body_len(&self) -> usize {
        self.body.len()
    }

    /// Add a header to the response in place (mutating).
    ///
    /// Use this after dispatch when the response has already been built by a handler
    /// and a cross-cutting header (e.g., Date) must be injected before writing.
    pub fn add_header(&mut self, name: &str, value: &str) {
        self.headers.push((name.to_string(), value.to_string()));
    }

    /// Serialize and write the response to the given writer.
    ///
    /// Format: "HTTP/1.1 {status} {reason}\r\n{headers}\r\n{body}"
    /// Each header line ends with CRLF. The header section ends with a blank CRLF line.
    pub fn write_to(self, writer: &mut impl Write) -> std::io::Result<()> {
        // Status line
        write!(writer, "HTTP/1.1 {} {}\r\n", self.status, self.reason)?;
        // Headers
        for (name, value) in &self.headers {
            write!(writer, "{}: {}\r\n", name, value)?;
        }
        // Blank separator line
        writer.write_all(b"\r\n")?;
        // Body
        if !self.body.is_empty() {
            writer.write_all(&self.body)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Response;

    #[test]
    fn status_line_crlf() {
        let mut buf: Vec<u8> = Vec::new();
        Response::new(200, "OK").write_to(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.starts_with("HTTP/1.1 200 OK\r\n"),
            "expected status line with CRLF, got: {:?}",
            output
        );
    }

    #[test]
    fn header_crlf() {
        let mut buf: Vec<u8> = Vec::new();
        Response::new(200, "OK")
            .header("Content-Type", "text/plain")
            .write_to(&mut buf)
            .unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.contains("Content-Type: text/plain\r\n"),
            "expected header line with CRLF, got: {:?}",
            output
        );
    }

    #[test]
    fn blank_separator() {
        let mut buf: Vec<u8> = Vec::new();
        Response::new(200, "OK")
            .header("Content-Length", "0")
            .write_to(&mut buf)
            .unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.contains("\r\n\r\n"),
            "expected blank separator line (CRLF CRLF), got: {:?}",
            output
        );
    }

    #[test]
    fn body_roundtrip_ascii() {
        let mut buf: Vec<u8> = Vec::new();
        Response::new(200, "OK")
            .header("Content-Length", "5")
            .body(b"hello".to_vec())
            .write_to(&mut buf)
            .unwrap();
        // the body appears after the blank separator
        let sep = b"\r\n\r\n";
        let sep_pos = buf
            .windows(4)
            .position(|w| w == sep)
            .expect("no blank separator");
        let body_part = &buf[sep_pos + 4..];
        assert_eq!(
            body_part, b"hello",
            "body did not round-trip: {:?}",
            body_part
        );
    }

    #[test]
    fn body_roundtrip_binary() {
        let binary_body = vec![0xFF_u8, 0x00, 0xAB];
        let mut buf: Vec<u8> = Vec::new();
        Response::new(200, "OK")
            .header("Content-Length", "3")
            .body(binary_body.clone())
            .write_to(&mut buf)
            .unwrap();
        let sep = b"\r\n\r\n";
        let sep_pos = buf
            .windows(4)
            .position(|w| w == sep)
            .expect("no blank separator");
        let body_part = &buf[sep_pos + 4..];
        assert_eq!(
            body_part,
            binary_body.as_slice(),
            "binary body did not round-trip: {:?}",
            body_part
        );
    }

    #[test]
    fn content_length_in_output() {
        let mut buf: Vec<u8> = Vec::new();
        Response::new(200, "OK")
            .header("Content-Type", "text/plain")
            .header("Content-Length", "2")
            .body(b"OK".to_vec())
            .write_to(&mut buf)
            .unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.contains("Content-Length: 2\r\n"),
            "expected Content-Length header, got: {:?}",
            output
        );
    }

    #[test]
    fn add_header_appends_header() {
        let mut response = Response::new(200, "OK");
        response.add_header("X-Test", "val");
        let mut buf: Vec<u8> = Vec::new();
        response.write_to(&mut buf).unwrap();
        let output = String::from_utf8(buf).unwrap();
        assert!(
            output.contains("X-Test: val\r\n"),
            "add_header did not append: {:?}",
            output
        );
    }

    #[test]
    fn status_accessor_returns_status_code() {
        assert_eq!(Response::new(200, "OK").status(), 200);
        assert_eq!(Response::new(404, "Not Found").status(), 404);
        assert_eq!(Response::new(500, "Internal Server Error").status(), 500);
    }

    #[test]
    fn body_len_accessor_returns_byte_count() {
        assert_eq!(Response::new(200, "OK").body_len(), 0);
        assert_eq!(
            Response::new(200, "OK").body(b"hello".to_vec()).body_len(),
            5
        );
        assert_eq!(Response::new(200, "OK").body(b"AB".to_vec()).body_len(), 2);
    }
}
