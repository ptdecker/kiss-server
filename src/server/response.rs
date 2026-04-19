//! HTTP Response -- re-exported from kiss-plugin-sdk.
pub use kiss_plugin_sdk::Response;

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
