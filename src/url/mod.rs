//! URL type -- re-exported from kiss-plugin-sdk.
#[allow(unused_imports)]
pub use kiss_plugin_sdk::Url;

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::from_utf8;

    // Private hex helper needed by pct_decode test helper (moved from module level)
    fn hex_char_to_byte(c: char) -> crate::Result<u8> {
        match c {
            '0'..='9' => Ok(c as u8 - b'0'),
            'a'..='f' => Ok(c as u8 - b'a' + 10),
            'A'..='F' => Ok(c as u8 - b'A' + 10),
            _ => Err(format!("The character '{}' is not a valid hexadecimal digit.", c).into()),
        }
    }

    // Helper function that encodes a UTF-8 Unicode character as an RTF-3986 Percent-Encoding (Cf.
    // section 2.1). Used only in tests.
    fn pct_encode(uni_char: char) -> String {
        let mut encoded = String::with_capacity(12);
        for byte in uni_char.to_string().as_bytes() {
            encoded.push_str(&format!("%{:02X}", byte));
        }
        encoded
    }

    // Helper function that takes a value stored in any type that can be referenced as a str.
    // If it contains an RTF 3986 Percent-Encoding of a valid UTF-8 Unicode character, that
    // character is returned as a char. Otherwise, an error is returned. Used only in tests.
    fn pct_decode<S: AsRef<str>>(pct_encoded: S) -> crate::Result<char> {
        let mut chars = pct_encoded.as_ref().chars().peekable();
        let mut bytes = Vec::<u8>::new();
        while let Some(c) = chars.next() {
            if c != '%' {
                return Err("expected '%' and didn't find it".into());
            }
            let mut next_byte = hex_char_to_byte(
                chars
                    .next()
                    .ok_or("invalid percentage encoding".to_string())?,
            )?;
            next_byte = (next_byte << 4)
                + hex_char_to_byte(
                    chars
                        .next()
                        .ok_or("invalid percentage encoding".to_string())?,
                )?;
            bytes.push(next_byte);
        }
        Ok(from_utf8(&bytes)
            .map_err(|_| "invalid UTF+8 unicode".to_string())?
            .chars()
            .next()
            .ok_or("pct-decode internal error")?)
    }

    #[test]
    fn path_strips_query() {
        assert_eq!(Url::from("/file.html?v=1").path(), "/file.html");
        assert_eq!(Url::from("/file.html").path(), "/file.html");
        assert_eq!(Url::from("/a?b=c&d=e").path(), "/a");
    }

    #[test]
    fn decoded_path_ascii() {
        let url = Url::from("/my%20file.html");
        assert_eq!(url.decoded_path().unwrap(), "/my file.html");
    }

    #[test]
    fn decoded_path_multibyte() {
        let url = Url::from("/%C3%A9.html");
        assert_eq!(url.decoded_path().unwrap(), "/\u{00E9}.html");
    }

    #[test]
    fn decoded_path_plain() {
        let url = Url::from("/plain.html");
        assert_eq!(url.decoded_path().unwrap(), "/plain.html");
    }

    #[test]
    fn decoded_path_invalid_hex() {
        let url = Url::from("/%GG");
        assert!(url.decoded_path().is_err());
    }

    #[test]
    fn decoded_path_truncated() {
        let url = Url::from("/%2");
        assert!(url.decoded_path().is_err());
    }

    #[test]
    fn decoded_path_strips_query_before_decode() {
        // query contains %20, but path does not; path() strips query first
        let url = Url::from("/path?query=%20");
        assert_eq!(url.decoded_path().unwrap(), "/path");
    }

    #[test]
    fn hex_byte() {
        const TEST_CHARS: [(u8, char); 22] = [
            (0, '0'),
            (1, '1'),
            (2, '2'),
            (3, '3'),
            (4, '4'),
            (5, '5'),
            (6, '6'),
            (7, '7'),
            (8, '8'),
            (9, '9'),
            (10, 'a'),
            (11, 'b'),
            (12, 'c'),
            (13, 'd'),
            (14, 'e'),
            (15, 'f'),
            (10, 'A'),
            (11, 'B'),
            (12, 'C'),
            (13, 'D'),
            (14, 'E'),
            (15, 'F'),
        ];
        for (value, char) in TEST_CHARS {
            assert_eq!(value, hex_char_to_byte(char).unwrap());
        }
    }

    #[test]
    fn encode() {
        assert_eq!("%65", pct_encode('\u{0065}')); // 1-byte UTF-8 glyph 'e'
        assert_eq!("%C3%A9", pct_encode('\u{00E9}')); // 2-byte UTF-8 glyph 'é'
        assert_eq!("%E2%82%AC", pct_encode('\u{20AC}')); // 3-byte UTF-8 glyph '€'
        assert_eq!("%F0%90%8D%88", pct_encode('\u{10348}')); // 4-byte UTF-8 glyph '𐍈'
    }

    #[test]
    fn decode() {
        assert_eq!('\u{0065}', pct_decode(pct_encode('\u{0065}')).unwrap()); // 1-byte UTF-8 glyph 'e'
        assert_eq!('\u{00E9}', pct_decode(pct_encode('\u{00E9}')).unwrap()); // 2-byte UTF-8 glyph 'é'
        assert_eq!('\u{20AC}', pct_decode(pct_encode('\u{20AC}')).unwrap()); // 3-byte UTF-8 glyph '€'
        assert_eq!('\u{10348}', pct_decode(pct_encode('\u{10348}')).unwrap()); // 4-byte UTF-8 glyph '𐍈'
    }
}
