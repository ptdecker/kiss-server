//! RFC 4648 §5 base64url encode/decode.
//!
//! * Alphabet index 62 is `-` (not `+`); index 63 is `_` (not `/`).
//! * No `=` padding is emitted or accepted — JWT parts never use padding.
//! * Hand-rolled with no external crates, per D-03.

// Phase 24 scope note: `pub fn encode` and `pub fn decode` are consumed by
// `src/jwt/` in Plans 02–04, but not reached from `main.rs` until Phase 25
// wires `AuthMiddleware` into the dispatch chain. Under Rust edition 2024,
// `dead_code` warnings fire on `pub` items even when used only from
// `#[cfg(test)]`, per 24-RESEARCH.md Pitfall 6. Suppress the lint at the
// module level; remove this attribute in Phase 25 when production code
// calls `base64::decode()`.
#![allow(dead_code)]

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// Encode bytes as base64url (no padding).
pub fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    let mut i = 0;
    while i + 3 <= bytes.len() {
        let b0 = bytes[i] as u32;
        let b1 = bytes[i + 1] as u32;
        let b2 = bytes[i + 2] as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[((triple >> 18) & 0x3f) as usize] as char);
        out.push(ALPHABET[((triple >> 12) & 0x3f) as usize] as char);
        out.push(ALPHABET[((triple >> 6) & 0x3f) as usize] as char);
        out.push(ALPHABET[(triple & 0x3f) as usize] as char);
        i += 3;
    }
    let rem = bytes.len() - i;
    if rem == 1 {
        let b0 = bytes[i] as u32;
        let triple = b0 << 16;
        out.push(ALPHABET[((triple >> 18) & 0x3f) as usize] as char);
        out.push(ALPHABET[((triple >> 12) & 0x3f) as usize] as char);
    } else if rem == 2 {
        let b0 = bytes[i] as u32;
        let b1 = bytes[i + 1] as u32;
        let triple = (b0 << 16) | (b1 << 8);
        out.push(ALPHABET[((triple >> 18) & 0x3f) as usize] as char);
        out.push(ALPHABET[((triple >> 12) & 0x3f) as usize] as char);
        out.push(ALPHABET[((triple >> 6) & 0x3f) as usize] as char);
    }
    out
}

/// Decode base64url (no padding). Rejects '=' and any non-alphabet byte.
pub fn decode(s: &str) -> Result<Vec<u8>, &'static str> {
    let bytes = s.as_bytes();
    // Length 1 is never valid (would decode to 0 bytes with 6 stray bits).
    if bytes.len() == 1 {
        return Err("base64url: invalid length");
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3 + 3);
    let mut buf: u32 = 0;
    let mut bits: u32 = 0;
    for &c in bytes {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            _ => return Err("base64url: invalid character"),
        } as u32;
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((buf >> bits) & 0xff) as u8);
        }
    }
    // Remaining bits (< 8) must all be zero — they're unused padding bits.
    // If there are >= 8 bits left or leftover bits are non-zero, the input was malformed.
    if bits > 0 {
        let mask = (1u32 << bits) - 1;
        if buf & mask != 0 {
            return Err("base64url: non-zero trailing bits");
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_empty() {
        assert_eq!(encode(&[]), "");
    }

    #[test]
    fn encode_one_byte() {
        assert_eq!(encode(&[0xff]), "_w");
    }

    #[test]
    fn encode_two_bytes() {
        assert_eq!(encode(&[0xff, 0xff]), "__8");
    }

    #[test]
    fn encode_three_bytes() {
        assert_eq!(encode(&[0xff, 0xff, 0xff]), "____");
    }

    #[test]
    fn encode_uses_dash_not_plus() {
        // 0xfb = 0b11111011 → top 6 bits = 0b111110 = 62 → '-'
        assert_eq!(encode(&[0xfb]), "-w");
    }

    #[test]
    fn encode_uses_underscore_not_slash() {
        // For [0xff, 0xe0]: 0xff 0xe0 = 1111_1111 1110_0000
        // 6-bit groups: 111111 (63='_'), 111110 (62='-'), 000000 (0='A')
        assert_eq!(encode(&[0xff, 0xe0]), "_-A");
    }

    #[test]
    fn decode_empty() {
        assert_eq!(decode("").unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn decode_rejects_padding() {
        assert!(decode("QQ==").is_err());
    }

    #[test]
    fn decode_rejects_plus() {
        assert!(decode("+A").is_err());
    }

    #[test]
    fn decode_rejects_slash() {
        assert!(decode("/A").is_err());
    }

    #[test]
    fn decode_rejects_invalid_char() {
        assert!(decode("!!").is_err());
    }

    #[test]
    fn decode_known_vector_hello() {
        assert_eq!(decode("SGVsbG8").unwrap(), b"Hello");
    }

    #[test]
    fn roundtrip_single_byte() {
        for b in [0x00u8, 0x01, 0x7f, 0x80, 0xff] {
            let input = vec![b];
            assert_eq!(decode(&encode(&input)).unwrap(), input);
        }
    }

    #[test]
    fn roundtrip_two_bytes() {
        let input = vec![0xab, 0xcd];
        assert_eq!(decode(&encode(&input)).unwrap(), input);
    }

    #[test]
    fn roundtrip_three_bytes() {
        let input = vec![0xab, 0xcd, 0xef];
        assert_eq!(decode(&encode(&input)).unwrap(), input);
    }

    #[test]
    fn roundtrip_all_byte_values() {
        let input: Vec<u8> = (0u8..=255).collect();
        assert_eq!(decode(&encode(&input)).unwrap(), input);
    }

    #[test]
    fn roundtrip_zero_padded() {
        let input = vec![0u8; 32];
        assert_eq!(decode(&encode(&input)).unwrap(), input);
    }
}
