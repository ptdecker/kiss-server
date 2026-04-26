//! JWT parsing, claims extraction, and RS256 signature verification.
//!
//! Staged API per Phase 24 D-01/D-02:
//!   - `parse(token: &str) -> Result<JwtParts, JwtError>`          (Plan 02)
//!   - `extract(payload_b64: &str) -> Result<JwtClaims, JwtError>` (Plan 03)
//!   - `verify(parts: &JwtParts, spki_der: &[u8]) -> Result<(), JwtError>` (Plan 04)
//!
//! All cryptographically sensitive operations delegate to `ring`. All parsing
//! (base64url, JWT splitting, JSON field extraction) is hand-rolled per D-03.
//!
//! # Dead-code note
//!
//! Under Rust edition 2024, `dead_code` warnings fire on `pub` items that are
//! not yet reachable from `main.rs`. AuthMiddleware is now wired into the
//! dispatch chain in Plan 05, making all jwt:: items reachable.

use std::fmt;

/// All errors surfaced by this module. A single shared error type keeps
/// `?`-chaining natural for Phase 25 consumers (per research §Error Design).
#[derive(Debug, PartialEq, Eq)]
pub enum JwtError {
    /// Token did not have exactly 3 dot-separated parts.
    MalformedToken,
    /// base64url decode of a JWT component failed.
    Base64DecodeError,
    /// Required claim is absent from the JSON payload.
    MissingClaim(&'static str),
    /// Claim is present but could not be parsed as the expected type.
    InvalidClaim(&'static str),
    /// The `exp` claim is in the past relative to `SystemTime::now()`.
    TokenExpired,
    /// The RSA public key bytes are not valid SPKI or RSAPublicKey DER.
    InvalidKey,
    /// The RS256 signature does not match the token's signed content.
    SignatureInvalid,
}

impl fmt::Display for JwtError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JwtError::MalformedToken => {
                write!(
                    f,
                    "jwt: malformed token (expected exactly 3 dot-separated parts)"
                )
            }
            JwtError::Base64DecodeError => write!(f, "jwt: base64url decode failed"),
            JwtError::MissingClaim(k) => write!(f, "jwt: missing required claim '{k}'"),
            JwtError::InvalidClaim(k) => write!(f, "jwt: invalid value for claim '{k}'"),
            JwtError::TokenExpired => write!(f, "jwt: token expired (exp in the past)"),
            JwtError::InvalidKey => {
                write!(
                    f,
                    "jwt: RSA public key bytes are not valid SPKI or RSAPublicKey DER"
                )
            }
            JwtError::SignatureInvalid => write!(f, "jwt: RS256 signature does not match"),
        }
    }
}

impl std::error::Error for JwtError {}

/// The three parts of a JWT.
///
/// `header_b64` and `payload_b64` retain their original base64url-encoded
/// strings because `verify()` must reconstruct the signing input
/// (`header_b64 + "." + payload_b64`) to check the RS256 signature.
/// The `signature` field holds the already-decoded raw signature bytes.
#[derive(Debug, PartialEq, Eq)]
pub struct JwtParts {
    pub header_b64: String,
    pub payload_b64: String,
    pub signature: Vec<u8>,
}

/// Standard JWT claims used by Auth0 authentication.
///
/// - `sub` — subject / user identity
/// - `exp` — expiration timestamp (seconds since Unix epoch)
/// - `iss` — issuer (Auth0 tenant URL)
/// - `aud` — audience (client identifier)
#[derive(Debug, PartialEq, Eq)]
pub struct JwtClaims {
    pub sub: String,
    pub exp: u64,
    pub iss: String,
    pub aud: String,
}

/// Parse a JWT string into its three components.
///
/// Splits on `.` to get `header_b64`, `payload_b64`, and `signature_b64`,
/// then base64url-decodes the signature. `header_b64` and `payload_b64`
/// are kept as-is because `verify()` must reconstruct the signing input
/// `"{header_b64}.{payload_b64}"` to check the signature.
///
/// # Errors
/// - `JwtError::MalformedToken` if the token does not contain exactly 2 dots
/// - `JwtError::Base64DecodeError` if the signature component cannot be
///   base64url-decoded
pub fn parse(token: &str) -> Result<JwtParts, JwtError> {
    // Find the two dot separators. Any other count (0, 1, 3+) is malformed.
    let first_dot = token.find('.').ok_or(JwtError::MalformedToken)?;
    let rest = &token[first_dot + 1..];
    let second_dot_rel = rest.find('.').ok_or(JwtError::MalformedToken)?;
    let second_dot = first_dot + 1 + second_dot_rel;

    let header_b64 = &token[..first_dot];
    let payload_b64 = &token[first_dot + 1..second_dot];
    let signature_b64 = &token[second_dot + 1..];

    // Reject any further dots — a well-formed JWT has exactly 3 parts.
    if signature_b64.contains('.') {
        return Err(JwtError::MalformedToken);
    }

    let signature =
        crate::base64::decode(signature_b64).map_err(|_| JwtError::Base64DecodeError)?;

    Ok(JwtParts {
        header_b64: header_b64.to_string(),
        payload_b64: payload_b64.to_string(),
        signature,
    })
}

/// Stage 3: base64url-decode the payload and extract the four required claims.
///
/// Validates that `exp` is in the future relative to `SystemTime::now()`.
/// Presence and parseability are checked for `sub`, `iss`, `aud`, but their
/// values are NOT validated against any configured value — Phase 25's
/// `AuthMiddleware` performs issuer/audience matching.
///
/// # Scanner limitations
///
/// The JSON scanner assumes compact output with no whitespace before `:`
/// (matches Auth0's JWT format). Claim string values must not contain
/// an escaped `"` (UUIDs, URLs, and short identifiers in Auth0 payloads
/// never do). A fully general JSON parser is intentionally not built per
/// D-03 (no crates.io JSON deps).
///
/// # Errors
/// - `JwtError::Base64DecodeError` if payload is not valid base64url
/// - `JwtError::InvalidClaim(_)` if decoded bytes are not UTF-8 or exp is non-numeric
/// - `JwtError::MissingClaim(name)` if any of sub/exp/iss/aud is absent
/// - `JwtError::TokenExpired` if exp is earlier than the current system time
pub fn extract(payload_b64: &str) -> Result<JwtClaims, JwtError> {
    let bytes = crate::base64::decode(payload_b64).map_err(|_| JwtError::Base64DecodeError)?;
    let json = std::str::from_utf8(&bytes).map_err(|_| JwtError::InvalidClaim("payload"))?;

    let sub = extract_string_claim(json, "sub")
        .ok_or(JwtError::MissingClaim("sub"))?
        .to_string();
    let iss = extract_string_claim(json, "iss")
        .ok_or(JwtError::MissingClaim("iss"))?
        .to_string();
    let aud = extract_aud_claim(json).ok_or(JwtError::MissingClaim("aud"))?;

    // exp is a numeric claim. If the key is absent → MissingClaim.
    // If the key is present but the value is not a parseable u64 → InvalidClaim.
    let exp_present = json.contains("\"exp\":");
    let exp = match extract_u64_claim(json, "exp") {
        Some(v) => v,
        None if exp_present => return Err(JwtError::InvalidClaim("exp")),
        None => return Err(JwtError::MissingClaim("exp")),
    };

    // Validate exp is in the future.
    // Use checked_add to handle exp values (e.g. u64::MAX) that would overflow
    // SystemTime arithmetic. An overflow means the timestamp is astronomically
    // far in the future — treat as valid (not expired).
    use std::time::{Duration, SystemTime, UNIX_EPOCH};
    let now = SystemTime::now();
    match UNIX_EPOCH.checked_add(Duration::from_secs(exp)) {
        Some(exp_time) if exp_time < now => return Err(JwtError::TokenExpired),
        _ => {}
    }

    Ok(JwtClaims { sub, exp, iss, aud })
}

/// Find `"key":"value"` and return the `value` slice (no escape handling).
///
/// Returns `None` if the key is absent or the value is not a quoted string.
///
/// # Limitations
///
/// Searches for the first occurrence of `"key":"` anywhere in `json`, including
/// inside the string value of a prior claim. Claim values must not contain the
/// substring `"<key>":"` for any claim key being extracted (e.g., the `sub` value
/// must not contain `"iss":"`). Auth0-issued tokens satisfy this constraint;
/// custom issuers may not.
fn extract_string_claim<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("\"{}\":\"", key);
    let start = json.find(&needle)? + needle.len();
    let end = json[start..].find('"')? + start;
    Some(&json[start..end])
}

/// Extract the `aud` claim, handling both plain string and single-element array forms.
///
/// Auth0 M2M tokens use `"aud":"<value>"` (plain string). SPA flows and multi-audience
/// grants use `"aud":["<value>"]` (JSON array). Both forms are valid per RFC 7519 §4.1.3.
/// This function accepts either and returns the audience value as an owned `String`.
///
/// Returns `None` if the `aud` key is absent entirely.
fn extract_aud_claim(json: &str) -> Option<String> {
    // Try plain string form first: "aud":"value"
    if let Some(v) = extract_string_claim(json, "aud") {
        return Some(v.to_string());
    }
    // Try single-element array form: "aud":["value"]
    let needle = "\"aud\":[\"";
    let start = json.find(needle)? + needle.len();
    let end = json[start..].find('"')? + start;
    Some(json[start..end].to_string())
}

/// Find `"key":<digits>` and parse to u64.
///
/// Returns `None` if the key is absent or the value is not purely digits.
fn extract_u64_claim(json: &str, key: &str) -> Option<u64> {
    let needle = format!("\"{}\":", key);
    let start = json.find(&needle)? + needle.len();
    // Reject string-typed values like "exp":"123" — the next char must be a digit.
    let rest = &json[start..];
    if !rest.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    let digits_end = rest
        .find(|c: char| !c.is_ascii_digit())
        .map(|n| start + n)
        .unwrap_or(json.len());
    json[start..digits_end].parse().ok()
}

/// Strip a SubjectPublicKeyInfo (SPKI) DER wrapper to get the inner
/// `RSAPublicKey` DER that `ring` requires.
///
/// SPKI structure for RSA (RFC 5280):
/// ```text
/// SEQUENCE {
///   SEQUENCE {                     -- algorithm identifier
///     OID 1.2.840.113549.1.1.1     -- rsaEncryption
///     NULL
///   }
///   BIT STRING {
///     0x00                          -- unused-bits indicator
///     SEQUENCE {                    -- RSAPublicKey (what we return)
///       INTEGER n
///       INTEGER e
///     }
///   }
/// }
/// ```
///
/// Returns `Err(JwtError::InvalidKey)` on any malformed or non-RSA SPKI.
fn spki_to_rsa_pubkey(spki: &[u8]) -> Result<&[u8], JwtError> {
    // Consume a DER TLV starting at `offset`. Returns (tag, content_start, content_end).
    fn read_tlv(bytes: &[u8], offset: usize) -> Result<(u8, usize, usize), JwtError> {
        if offset >= bytes.len() {
            return Err(JwtError::InvalidKey);
        }
        let tag = bytes[offset];
        if offset + 1 >= bytes.len() {
            return Err(JwtError::InvalidKey);
        }
        let len_byte = bytes[offset + 1];
        let (content_start, content_len) = if len_byte < 0x80 {
            (offset + 2, len_byte as usize)
        } else {
            let n = (len_byte & 0x7f) as usize;
            if n == 0 || n > 4 {
                return Err(JwtError::InvalidKey);
            }
            if offset + 2 + n > bytes.len() {
                return Err(JwtError::InvalidKey);
            }
            let mut len = 0usize;
            for i in 0..n {
                len = (len << 8) | (bytes[offset + 2 + i] as usize);
            }
            (offset + 2 + n, len)
        };
        let content_end = content_start
            .checked_add(content_len)
            .ok_or(JwtError::InvalidKey)?;
        if content_end > bytes.len() {
            return Err(JwtError::InvalidKey);
        }
        Ok((tag, content_start, content_end))
    }

    // Outer SEQUENCE
    let (tag, outer_start, outer_end) = read_tlv(spki, 0)?;
    if tag != 0x30 {
        return Err(JwtError::InvalidKey);
    }

    // Algorithm identifier SEQUENCE — skip it
    let (alg_tag, _alg_start, alg_end) = read_tlv(spki, outer_start)?;
    if alg_tag != 0x30 {
        return Err(JwtError::InvalidKey);
    }
    // Algorithm identifier must fit inside the outer SEQUENCE.
    if alg_end > outer_end {
        return Err(JwtError::InvalidKey);
    }

    // BIT STRING containing the subjectPublicKey
    let (bs_tag, bs_start, bs_end) = read_tlv(spki, alg_end)?;
    if bs_tag != 0x03 {
        return Err(JwtError::InvalidKey);
    }
    if bs_end > outer_end {
        return Err(JwtError::InvalidKey);
    }

    // First byte of BIT STRING content is the "unused bits" count — must be 0 for RSA.
    if bs_start >= spki.len() || spki[bs_start] != 0x00 {
        return Err(JwtError::InvalidKey);
    }

    // Remaining bytes are the RSAPublicKey DER — must start with SEQUENCE tag.
    let rsa_start = bs_start + 1;
    if rsa_start >= bs_end {
        return Err(JwtError::InvalidKey);
    }
    if spki[rsa_start] != 0x30 {
        return Err(JwtError::InvalidKey);
    }

    Ok(&spki[rsa_start..bs_end])
}

/// Wrap an RSAPublicKey DER slice in a SPKI envelope.
///
/// Used by `src/jwks/mod.rs` to package the modulus + exponent extracted from a
/// JWKS JSON response into the SPKI format that `verify()` accepts.
/// Also used in `#[cfg(test)]` to construct SPKI fixtures from RSAPublicKey DER.
pub(crate) fn wrap_rsa_pubkey_as_spki(rsa_pubkey: &[u8]) -> Vec<u8> {
    // Algorithm identifier: SEQUENCE { OID 1.2.840.113549.1.1.1, NULL }
    //   30 0D 06 09 2A 86 48 86 F7 0D 01 01 01 05 00  (15 bytes)
    const ALG_ID: [u8; 15] = [
        0x30, 0x0D, 0x06, 0x09, 0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x01, 0x01, 0x05, 0x00,
    ];

    // Build BIT STRING: 0x03 <len-encoding> 0x00 <rsa_pubkey>
    let bs_content_len = 1 + rsa_pubkey.len();
    let bs_len_encoding = encode_der_length(bs_content_len);
    let mut bit_string = Vec::with_capacity(1 + bs_len_encoding.len() + 1 + rsa_pubkey.len());
    bit_string.push(0x03);
    bit_string.extend_from_slice(&bs_len_encoding);
    bit_string.push(0x00);
    bit_string.extend_from_slice(rsa_pubkey);

    // Outer SEQUENCE: 0x30 <len-encoding> <alg_id> <bit_string>
    let inner_len = ALG_ID.len() + bit_string.len();
    let outer_len_encoding = encode_der_length(inner_len);
    let mut spki = Vec::with_capacity(1 + outer_len_encoding.len() + inner_len);
    spki.push(0x30);
    spki.extend_from_slice(&outer_len_encoding);
    spki.extend_from_slice(&ALG_ID);
    spki.extend_from_slice(&bit_string);
    spki
}

/// Encode a length for DER TLV. Short form (< 128) is one byte;
/// long form uses `0x80 | n` + n bytes big-endian. Supports up to 3-byte lengths
/// (0xFFFFFF), which is far more than any RSA 2048/4096 SPKI requires.
pub(crate) fn encode_der_length(len: usize) -> Vec<u8> {
    if len < 0x80 {
        vec![len as u8]
    } else if len < 0x100 {
        vec![0x81, len as u8]
    } else if len < 0x10000 {
        vec![0x82, (len >> 8) as u8, len as u8]
    } else if len < 0x1000000 {
        vec![0x83, (len >> 16) as u8, (len >> 8) as u8, len as u8]
    } else {
        // Defensive: an RSA SPKI larger than 16 MB is not a real-world key.
        // Cap at the 3-byte form rather than panicking; callers will produce
        // a malformed DER if they actually need this, and ring will reject.
        vec![0x83, 0xFF, 0xFF, 0xFF]
    }
}

/// Stage 2: verify the RS256 signature over the JWT's signing input.
///
/// Accepts the RSA public key as **SubjectPublicKeyInfo (SPKI) DER bytes** —
/// the format produced by parsing a JWKS `kty:"RSA"` entry in Phase 25.
/// Internally strips the SPKI wrapper to the RSAPublicKey format that
/// `ring` requires.
///
/// # Algorithm
/// RS256 (RSASSA-PKCS1-v1_5 with SHA-256) via `ring`.
/// Keys smaller than 2048 bits or larger than 8192 bits are rejected.
///
/// # Algorithm confusion prevention
/// This function reads the JWT header's `alg` field and rejects any token
/// that does not declare `"alg":"RS256"`. This makes the defense unconditional
/// and removes the need for callers to validate the algorithm themselves.
///
/// # Errors
/// - `JwtError::Base64DecodeError` if the header component is not valid base64url
/// - `JwtError::MalformedToken` if the header bytes are not valid UTF-8
/// - `JwtError::MissingClaim("alg")` if the header lacks an `alg` field
/// - `JwtError::SignatureInvalid` if `alg != "RS256"`, signature does not match,
///   key is wrong, or key size is outside the accepted range
/// - `JwtError::InvalidKey` if `spki_der` is not parseable SPKI DER
pub fn verify(parts: &JwtParts, spki_der: &[u8]) -> Result<(), JwtError> {
    // Reject non-RS256 alg unconditionally to prevent algorithm confusion attacks.
    let header_bytes =
        crate::base64::decode(&parts.header_b64).map_err(|_| JwtError::Base64DecodeError)?;
    let header_json = std::str::from_utf8(&header_bytes).map_err(|_| JwtError::MalformedToken)?;
    let alg = extract_string_claim(header_json, "alg").ok_or(JwtError::MissingClaim("alg"))?;
    if alg != "RS256" {
        return Err(JwtError::SignatureInvalid);
    }

    // Strip the SPKI wrapper — ring's UnparsedPublicKey for RSA requires
    // RSAPublicKey DER (inner SEQUENCE of modulus+exponent), not SPKI.
    let rsa_pubkey = spki_to_rsa_pubkey(spki_der)?;

    // Reconstruct the signing input: header_b64 + "." + payload_b64.
    // CRITICAL: use the original base64url-encoded strings, NOT decoded bytes.
    let mut signing_input =
        Vec::with_capacity(parts.header_b64.len() + 1 + parts.payload_b64.len());
    signing_input.extend_from_slice(parts.header_b64.as_bytes());
    signing_input.push(b'.');
    signing_input.extend_from_slice(parts.payload_b64.as_bytes());

    let public_key = ring::signature::UnparsedPublicKey::new(
        &ring::signature::RSA_PKCS1_2048_8192_SHA256,
        rsa_pubkey,
    );

    public_key
        .verify(&signing_input, &parts.signature)
        .map_err(|_| JwtError::SignatureInvalid)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    // --- Task 1: Type surface smoke tests ---

    #[test]
    fn jwt_error_display_includes_type_name() {
        assert!(format!("{}", JwtError::MalformedToken).contains("malformed"));
        assert!(format!("{}", JwtError::MissingClaim("sub")).contains("sub"));
    }

    #[test]
    fn jwt_error_implements_std_error() {
        fn assert_error<E: std::error::Error>() {}
        assert_error::<JwtError>();
    }

    #[test]
    fn jwt_parts_fields_are_public() {
        let p = JwtParts {
            header_b64: "aaa".to_string(),
            payload_b64: "bbb".to_string(),
            signature: vec![0, 1, 2],
        };
        assert_eq!(p.header_b64, "aaa");
        assert_eq!(p.payload_b64, "bbb");
        assert_eq!(p.signature, vec![0, 1, 2]);
    }

    #[test]
    fn jwt_claims_fields_are_public() {
        let c = JwtClaims {
            sub: "user123".to_string(),
            exp: 1_700_000_000,
            iss: "https://example.auth0.com/".to_string(),
            aud: "myapp".to_string(),
        };
        assert_eq!(c.sub, "user123");
        assert_eq!(c.exp, 1_700_000_000);
    }

    // --- Task 2: parse() tests ---

    #[test]
    fn parse_valid_three_part_token() {
        // "Zm9v" is base64url for b"foo"
        let parts = parse("aaa.bbb.Zm9v").unwrap();
        assert_eq!(parts.header_b64, "aaa");
        assert_eq!(parts.payload_b64, "bbb");
        assert_eq!(parts.signature, b"foo");
    }

    #[test]
    fn parse_rejects_no_dots() {
        assert_eq!(parse("abc"), Err(JwtError::MalformedToken));
    }

    #[test]
    fn parse_rejects_one_dot() {
        assert_eq!(parse("aaa.bbb"), Err(JwtError::MalformedToken));
    }

    #[test]
    fn parse_rejects_three_dots() {
        assert_eq!(parse("a.b.c.d"), Err(JwtError::MalformedToken));
    }

    #[test]
    fn parse_rejects_empty_string() {
        assert_eq!(parse(""), Err(JwtError::MalformedToken));
    }

    #[test]
    fn parse_rejects_invalid_base64_signature() {
        assert_eq!(parse("aaa.bbb.!!!"), Err(JwtError::Base64DecodeError));
    }

    #[test]
    fn parse_preserves_canonical_header_payload() {
        // The verify() function will later reconstruct "{header_b64}.{payload_b64}"
        // as the signing input. Any re-encoding here would break signature validation.
        let original = "eyJhbGciOiJSUzI1NiJ9.eyJzdWIiOiJ1c2VyIn0.Zm9v";
        let parts = parse(original).unwrap();
        assert_eq!(parts.header_b64, "eyJhbGciOiJSUzI1NiJ9");
        assert_eq!(parts.payload_b64, "eyJzdWIiOiJ1c2VyIn0");
    }

    #[test]
    fn parse_allows_empty_signature() {
        let parts = parse("aaa.bbb.").unwrap();
        assert_eq!(parts.signature, Vec::<u8>::new());
    }

    #[test]
    fn parse_does_not_panic_on_unicode() {
        // Must return Err, not panic, even on UTF-8 content between dots.
        // Note: Japanese/Chinese chars are not base64url alphabet → Base64DecodeError
        // once the split produces three parts. If fewer than 2 dots, MalformedToken.
        let result = parse("日本語.中国語.ñ");
        assert!(
            result.is_err(),
            "must return Err, not panic, on unicode input"
        );
    }

    // --- Plan 03: extract() tests ---

    // Helper: build a base64url-encoded payload from raw JSON.
    fn payload_b64(json: &str) -> String {
        crate::base64::encode(json.as_bytes())
    }

    #[test]
    fn extract_returns_all_four_claims() {
        let json = r#"{"sub":"user123","exp":18446744073709551615,"iss":"https://example.auth0.com/","aud":"myapp"}"#;
        let claims = extract(&payload_b64(json)).unwrap();
        assert_eq!(claims.sub, "user123");
        assert_eq!(claims.exp, u64::MAX);
        assert_eq!(claims.iss, "https://example.auth0.com/");
        assert_eq!(claims.aud, "myapp");
    }

    #[test]
    fn extract_missing_sub_returns_err() {
        let json = r#"{"exp":18446744073709551615,"iss":"x","aud":"y"}"#;
        assert_eq!(
            extract(&payload_b64(json)),
            Err(JwtError::MissingClaim("sub"))
        );
    }

    #[test]
    fn extract_missing_exp_returns_err() {
        let json = r#"{"sub":"u","iss":"x","aud":"y"}"#;
        assert_eq!(
            extract(&payload_b64(json)),
            Err(JwtError::MissingClaim("exp"))
        );
    }

    #[test]
    fn extract_missing_iss_returns_err() {
        let json = r#"{"sub":"u","exp":18446744073709551615,"aud":"y"}"#;
        assert_eq!(
            extract(&payload_b64(json)),
            Err(JwtError::MissingClaim("iss"))
        );
    }

    #[test]
    fn extract_missing_aud_returns_err() {
        let json = r#"{"sub":"u","exp":18446744073709551615,"iss":"x"}"#;
        assert_eq!(
            extract(&payload_b64(json)),
            Err(JwtError::MissingClaim("aud"))
        );
    }

    #[test]
    fn extract_exp_in_past_returns_token_expired() {
        // exp=1 is Jan 1 1970 00:00:01 UTC — always in the past.
        let json = r#"{"sub":"u","exp":1,"iss":"x","aud":"y"}"#;
        assert_eq!(extract(&payload_b64(json)), Err(JwtError::TokenExpired));
    }

    #[test]
    fn extract_exp_not_parseable_returns_invalid_claim() {
        let json = r#"{"sub":"u","exp":"not-a-number","iss":"x","aud":"y"}"#;
        assert_eq!(
            extract(&payload_b64(json)),
            Err(JwtError::InvalidClaim("exp"))
        );
    }

    #[test]
    fn extract_invalid_base64_returns_base64_decode_error() {
        assert_eq!(extract("!!!"), Err(JwtError::Base64DecodeError));
    }

    #[test]
    fn extract_invalid_utf8_returns_err() {
        // base64url of [0xff, 0xfe, 0xfd] — not valid UTF-8
        let payload = crate::base64::encode(&[0xff, 0xfe, 0xfd]);
        assert!(extract(&payload).is_err());
    }

    #[test]
    fn extract_accepts_arbitrary_iss_and_aud_values() {
        // Phase 24 does NOT validate iss/aud values — only presence.
        let json = r#"{"sub":"u","exp":18446744073709551615,"iss":"anything","aud":"whatever"}"#;
        let claims = extract(&payload_b64(json)).unwrap();
        assert_eq!(claims.iss, "anything");
        assert_eq!(claims.aud, "whatever");
    }

    #[test]
    fn extract_accepts_array_aud_single_element() {
        // Auth0 SPA/multi-audience grants use "aud":["value"] — must be accepted.
        let json = r#"{"sub":"u","exp":18446744073709551615,"iss":"x","aud":["myapp"]}"#;
        let claims = extract(&payload_b64(json)).unwrap();
        assert_eq!(claims.aud, "myapp");
    }

    #[test]
    fn extract_array_aud_missing_returns_err() {
        // No aud field at all — still MissingClaim, even with array syntax absent.
        let json = r#"{"sub":"u","exp":18446744073709551615,"iss":"x"}"#;
        assert_eq!(
            extract(&payload_b64(json)),
            Err(JwtError::MissingClaim("aud"))
        );
    }

    #[test]
    fn extract_accepts_large_exp_values() {
        let json = r#"{"sub":"u","exp":18446744073709551615,"iss":"x","aud":"y"}"#;
        let claims = extract(&payload_b64(json)).unwrap();
        assert_eq!(claims.exp, u64::MAX);
    }

    #[test]
    fn extract_does_not_panic_on_malformed_json() {
        let malformed = crate::base64::encode(b"{not json");
        let result = extract(&malformed);
        assert!(result.is_err(), "malformed JSON must return Err, not panic");
    }

    // --- Plan 04: spki_to_rsa_pubkey tests ---

    #[test]
    fn spki_strip_rejects_empty() {
        assert_eq!(spki_to_rsa_pubkey(&[]), Err(JwtError::InvalidKey));
    }

    #[test]
    fn spki_strip_rejects_truncated() {
        assert_eq!(spki_to_rsa_pubkey(&[0x30]), Err(JwtError::InvalidKey));
    }

    #[test]
    fn spki_strip_rejects_wrong_outer_tag() {
        assert_eq!(
            spki_to_rsa_pubkey(&[0x02, 0x01, 0x00]),
            Err(JwtError::InvalidKey)
        );
    }

    #[test]
    fn spki_strip_rejects_missing_bit_string() {
        // Outer SEQUENCE containing just an algorithm identifier, no BIT STRING.
        // Outer: 30 0D, Inner alg: 30 0B 06 09 2a 86 48 86 f7 0d 01 01 01 — no BIT STRING follows.
        let malformed = [
            0x30, 0x0D, // outer SEQUENCE, 13 content bytes
            0x30, 0x0B, // inner SEQUENCE (alg id), 11 content bytes
            0x06, 0x09, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x01,
            0x01, // OID (11 bytes)
        ];
        assert_eq!(spki_to_rsa_pubkey(&malformed), Err(JwtError::InvalidKey));
    }

    #[test]
    fn spki_strip_synthetic_input_returns_inner_sequence() {
        // Build a minimal synthetic SPKI around a fake RSAPublicKey.
        // Fake RSAPublicKey: SEQUENCE { INTEGER 0x01, INTEGER 0x03 }
        //   30 06 02 01 01 02 01 03   (8 bytes total)
        let rsa_pubkey = [0x30, 0x06, 0x02, 0x01, 0x01, 0x02, 0x01, 0x03];

        // Algorithm identifier: SEQUENCE { OID rsaEncryption, NULL }
        //   30 0D 06 09 2A 86 48 86 F7 0D 01 01 01 05 00   (15 bytes total)
        let alg_id = [
            0x30, 0x0D, 0x06, 0x09, 0x2A, 0x86, 0x48, 0x86, 0xF7, 0x0D, 0x01, 0x01, 0x01, 0x05,
            0x00,
        ];

        // BIT STRING wrapper: 03 <len> 00 <rsa_pubkey bytes>
        //   content length = 1 (the 0x00) + rsa_pubkey.len() = 9
        let bit_string_content_len = 1 + rsa_pubkey.len(); // 9
        let mut bit_string = vec![0x03, bit_string_content_len as u8, 0x00];
        bit_string.extend_from_slice(&rsa_pubkey);

        // Outer SEQUENCE: 30 <len> <alg_id> <bit_string>
        let mut inner = Vec::new();
        inner.extend_from_slice(&alg_id);
        inner.extend_from_slice(&bit_string);
        let mut spki = vec![0x30, inner.len() as u8];
        spki.extend_from_slice(&inner);

        let stripped = spki_to_rsa_pubkey(&spki).unwrap();
        assert_eq!(stripped, &rsa_pubkey);
    }

    // --- Plan 04: verify() tests ---
    //
    // Per research finding #1: ring has NO RSA keygen API. We embed a
    // 2048-bit PKCS8 DER private key as a const and derive the public key
    // from it. Each test signs a synthetic JWT and calls verify().

    // NOTE: Generated via:
    //   openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:2048 -pkeyopt rsa_keygen_pubexp:65537 \
    //     -outform der 2>/dev/null | openssl pkcs8 -topk8 -nocrypt -outform der | xxd -i
    pub(crate) const TEST_PRIVATE_KEY_PKCS8_DER: &[u8] = &[
        0x30, 0x82, 0x04, 0xbd, 0x02, 0x01, 0x00, 0x30, 0x0d, 0x06, 0x09, 0x2a, 0x86, 0x48, 0x86,
        0xf7, 0x0d, 0x01, 0x01, 0x01, 0x05, 0x00, 0x04, 0x82, 0x04, 0xa7, 0x30, 0x82, 0x04, 0xa3,
        0x02, 0x01, 0x00, 0x02, 0x82, 0x01, 0x01, 0x00, 0x9e, 0x1f, 0x20, 0xa3, 0x50, 0x3e, 0xb3,
        0x11, 0xcf, 0xf2, 0x23, 0x63, 0xd9, 0xa9, 0x99, 0x10, 0x82, 0x95, 0xe5, 0x52, 0x76, 0xf8,
        0xf2, 0x4e, 0x63, 0x52, 0x16, 0xfc, 0x3a, 0xa6, 0x90, 0xca, 0x4a, 0x03, 0x00, 0x0f, 0xf6,
        0x59, 0xdf, 0x48, 0x97, 0x23, 0xc1, 0x78, 0xac, 0x8c, 0x66, 0xc6, 0x8a, 0x65, 0x9d, 0xae,
        0xd0, 0x6e, 0xb7, 0x89, 0xc5, 0x47, 0x15, 0x33, 0x66, 0xe7, 0x3e, 0x89, 0x75, 0xf2, 0xf9,
        0x66, 0x36, 0x93, 0x1e, 0xdd, 0x06, 0xde, 0x2d, 0x8e, 0x3b, 0xf6, 0x0f, 0x48, 0x26, 0xdd,
        0x4b, 0x31, 0x92, 0x2f, 0x4b, 0xc6, 0xd6, 0xfb, 0x57, 0xc3, 0xa4, 0x7f, 0x5a, 0x5e, 0x78,
        0x7f, 0x5f, 0xa4, 0x34, 0x9f, 0x17, 0x50, 0x2f, 0xda, 0xe5, 0xcd, 0x7e, 0xc7, 0xd5, 0x8b,
        0x1e, 0x07, 0x90, 0xf7, 0x86, 0x88, 0xec, 0x8e, 0x86, 0x29, 0x22, 0xaa, 0x01, 0x07, 0xba,
        0xfe, 0xa6, 0x38, 0xdb, 0x60, 0xb4, 0x95, 0x7f, 0x04, 0x53, 0xcd, 0x40, 0xd8, 0x2f, 0x0b,
        0xfa, 0x28, 0xa2, 0x6f, 0xee, 0xed, 0x9f, 0x63, 0xd6, 0x8e, 0x4b, 0xa7, 0xf8, 0x11, 0xfe,
        0xd6, 0x97, 0x2c, 0x38, 0x65, 0xb7, 0x52, 0xea, 0x74, 0x83, 0x41, 0xaf, 0xdd, 0xe3, 0x6c,
        0x5a, 0xa7, 0xe4, 0x33, 0x03, 0x40, 0xca, 0x48, 0xfe, 0x71, 0x17, 0x1c, 0xf5, 0xe2, 0xa9,
        0x8d, 0xa6, 0x8f, 0x55, 0xd6, 0x89, 0x29, 0x9d, 0xd8, 0xa9, 0x90, 0x6a, 0xcc, 0xea, 0x4b,
        0x12, 0x3a, 0xe7, 0x66, 0x3b, 0x19, 0x59, 0x2d, 0x5d, 0x2a, 0x03, 0x4c, 0x3c, 0xbf, 0xd6,
        0x49, 0x2f, 0x83, 0x97, 0x45, 0xad, 0xa3, 0x24, 0x55, 0xa0, 0xb6, 0x40, 0x07, 0xe7, 0xb1,
        0xd5, 0x79, 0xd6, 0xb8, 0xcc, 0xf5, 0x31, 0xeb, 0x63, 0x11, 0xa3, 0x9f, 0x8c, 0x69, 0xed,
        0x00, 0x79, 0x54, 0xf5, 0x24, 0x5d, 0x37, 0x43, 0xf3, 0x02, 0x03, 0x01, 0x00, 0x01, 0x02,
        0x82, 0x01, 0x00, 0x0c, 0x98, 0x75, 0x2c, 0x9d, 0x35, 0x49, 0x65, 0x67, 0x57, 0xa5, 0xfb,
        0x0b, 0xb9, 0x27, 0x50, 0xb9, 0xde, 0x3f, 0xe4, 0xb8, 0x46, 0x6f, 0x62, 0xd0, 0x25, 0x97,
        0x60, 0x23, 0x39, 0xe6, 0x79, 0xbe, 0xb8, 0x8b, 0x76, 0x3a, 0x0d, 0x01, 0xac, 0xd2, 0x21,
        0x4e, 0x85, 0x92, 0xd5, 0x22, 0x50, 0xb9, 0x70, 0x8b, 0xc5, 0x83, 0xa3, 0xf6, 0x29, 0xda,
        0xd5, 0xa5, 0x3a, 0x31, 0x7d, 0xc5, 0x2d, 0x05, 0x00, 0xc4, 0x5e, 0x20, 0x85, 0xca, 0xa5,
        0x17, 0xa9, 0x76, 0x1e, 0x88, 0xe7, 0x3a, 0x63, 0x68, 0x79, 0x23, 0x46, 0xc1, 0x1a, 0xd4,
        0x61, 0xf3, 0xf9, 0x14, 0x8e, 0x28, 0x99, 0xde, 0x74, 0x7d, 0x0b, 0xc4, 0x6e, 0x1b, 0xa4,
        0xfa, 0xa1, 0xf0, 0x15, 0xf3, 0x4d, 0x51, 0xc6, 0x90, 0x5b, 0xa3, 0x1a, 0x3e, 0x31, 0x81,
        0x7d, 0x81, 0xbe, 0x7e, 0xb0, 0x63, 0x2b, 0x00, 0xa7, 0x01, 0x7d, 0xac, 0x57, 0x8f, 0x33,
        0x09, 0xca, 0x8e, 0x44, 0x93, 0x95, 0x62, 0x30, 0xf0, 0x03, 0xfb, 0xfd, 0x74, 0x4f, 0x0f,
        0xf8, 0xb2, 0xec, 0xd1, 0x59, 0x53, 0xd2, 0xd3, 0x76, 0x3e, 0x02, 0xf9, 0xc9, 0xdc, 0x38,
        0xe9, 0xf8, 0xd2, 0xfa, 0xa9, 0xc3, 0x5c, 0x8f, 0x5b, 0x27, 0x05, 0x25, 0xdc, 0xdd, 0x25,
        0x42, 0x7d, 0x4c, 0x4f, 0x15, 0x07, 0x59, 0x7a, 0xe7, 0x25, 0x4e, 0x29, 0xef, 0x42, 0xb1,
        0x59, 0xcf, 0xaf, 0x98, 0x02, 0x1b, 0x98, 0x8d, 0x77, 0x62, 0x59, 0x7b, 0xfc, 0xd8, 0xfa,
        0x8f, 0xef, 0x75, 0x4b, 0x45, 0x19, 0x35, 0x14, 0x4c, 0x25, 0xfe, 0x15, 0x7f, 0x14, 0x57,
        0x91, 0xc1, 0x4a, 0x05, 0x26, 0x72, 0x27, 0x3c, 0xa8, 0x11, 0x45, 0x0e, 0x0b, 0x49, 0x9c,
        0x63, 0x02, 0x44, 0x25, 0x47, 0x7f, 0x00, 0x7a, 0x0c, 0x98, 0xf5, 0x48, 0xa2, 0x5a, 0xb4,
        0xd0, 0x94, 0x7f, 0xfd, 0x02, 0x81, 0x81, 0x00, 0xd6, 0xdc, 0x38, 0x75, 0x2c, 0xd5, 0x28,
        0x80, 0xc9, 0x0f, 0xa3, 0x23, 0x35, 0x7f, 0xb3, 0x92, 0x12, 0x47, 0x27, 0xdd, 0x39, 0xec,
        0x0f, 0xdb, 0x29, 0x7d, 0x11, 0x4c, 0x4d, 0x7c, 0xa4, 0x74, 0x60, 0xe6, 0x78, 0x04, 0x58,
        0xb3, 0x77, 0xef, 0x16, 0xc1, 0x94, 0x53, 0xf0, 0x97, 0x10, 0x0f, 0x0c, 0xf6, 0x67, 0x1e,
        0x64, 0x8f, 0x71, 0x60, 0xb1, 0xcf, 0x23, 0xe2, 0xff, 0x82, 0x96, 0xa0, 0x00, 0xb8, 0x23,
        0xac, 0x3f, 0x3b, 0x4c, 0x7a, 0x23, 0x1c, 0xe4, 0x15, 0x04, 0xc7, 0x41, 0x88, 0xdc, 0x25,
        0x27, 0x19, 0x81, 0x7c, 0x4b, 0xc4, 0x43, 0x3b, 0x5f, 0x6b, 0xcf, 0x3d, 0xf2, 0x81, 0xa7,
        0x36, 0x0a, 0x3d, 0x64, 0x34, 0x3f, 0xd3, 0x93, 0x6b, 0xf5, 0x14, 0x4a, 0x23, 0xf1, 0x3d,
        0x51, 0xfa, 0x5f, 0x64, 0x42, 0x54, 0x2c, 0x85, 0xe7, 0x3f, 0x0e, 0x42, 0x00, 0xa8, 0x2d,
        0x07, 0x02, 0x81, 0x81, 0x00, 0xbc, 0x65, 0xc1, 0x5e, 0x15, 0x0c, 0x51, 0x8d, 0xf8, 0x3e,
        0x46, 0xde, 0xd1, 0x70, 0x6c, 0x14, 0x8c, 0xd9, 0x33, 0x59, 0xb6, 0x60, 0xfc, 0x1e, 0x71,
        0x86, 0xf4, 0x8c, 0x64, 0xd1, 0x56, 0xef, 0x27, 0xce, 0x94, 0x92, 0x44, 0x98, 0x22, 0x96,
        0xf7, 0x07, 0xf8, 0xf7, 0x97, 0xf2, 0x2e, 0x00, 0x4e, 0x48, 0x1a, 0xb2, 0xee, 0x5b, 0x3d,
        0x36, 0x2f, 0x66, 0x9b, 0x1a, 0xf6, 0x07, 0x3f, 0x9c, 0xb3, 0xca, 0x63, 0x04, 0xaf, 0x65,
        0x89, 0xc2, 0xe7, 0xc0, 0x12, 0xb8, 0x32, 0x5b, 0x87, 0x3c, 0xe0, 0xe8, 0xfc, 0xd7, 0xd2,
        0x26, 0xc3, 0x0c, 0x48, 0x2f, 0xd8, 0x6b, 0xd6, 0xe7, 0xe0, 0x5b, 0x70, 0x02, 0x43, 0xda,
        0x88, 0x46, 0xbb, 0x50, 0xae, 0x50, 0x42, 0xe6, 0x9a, 0xdd, 0x48, 0xaa, 0x5c, 0xb9, 0x65,
        0x23, 0x16, 0x3f, 0xad, 0xb4, 0x09, 0xba, 0xeb, 0xb4, 0xc4, 0x67, 0xa2, 0xb5, 0x02, 0x81,
        0x80, 0x29, 0x88, 0x0f, 0xf1, 0xb6, 0x64, 0xcd, 0x9b, 0x77, 0x41, 0xea, 0x8a, 0xd7, 0xc0,
        0x83, 0x79, 0x6c, 0xc7, 0x0c, 0x51, 0x9a, 0xec, 0xa2, 0x73, 0xfe, 0xa5, 0x0a, 0x3e, 0xf1,
        0x8b, 0x72, 0x4e, 0x7c, 0x9c, 0x8f, 0xfe, 0x67, 0x16, 0xe9, 0xcb, 0xf1, 0x5e, 0x21, 0xc9,
        0xc7, 0xeb, 0xab, 0x52, 0xfd, 0x72, 0x73, 0xa4, 0x50, 0x53, 0xd9, 0xda, 0x93, 0x04, 0x33,
        0x2f, 0xa1, 0xac, 0x20, 0x69, 0x75, 0x3a, 0x22, 0xcb, 0x1c, 0xbd, 0xdd, 0x9e, 0x8e, 0x42,
        0xfb, 0x63, 0x84, 0xb4, 0xef, 0x5a, 0x01, 0x13, 0xbd, 0x67, 0x14, 0xbc, 0x6d, 0xf8, 0xd5,
        0xf6, 0x18, 0x0f, 0xc2, 0xd3, 0x7a, 0x98, 0xcd, 0x35, 0x88, 0xed, 0x2c, 0xfd, 0x5c, 0x89,
        0x0d, 0x2a, 0x05, 0x09, 0x92, 0xfb, 0x37, 0x9a, 0x5e, 0xca, 0x42, 0xbe, 0x22, 0x84, 0x1a,
        0xc7, 0x17, 0x57, 0xfc, 0xed, 0x8d, 0x2a, 0xf4, 0xe9, 0x02, 0x81, 0x81, 0x00, 0x84, 0x4e,
        0x8d, 0xcb, 0x0d, 0xcb, 0x15, 0xe7, 0x37, 0x24, 0x3d, 0x4f, 0x34, 0x14, 0xd8, 0xc2, 0x61,
        0xdc, 0x13, 0x94, 0xf8, 0x61, 0x0a, 0x0e, 0x33, 0x3a, 0x4c, 0xb9, 0xdf, 0xff, 0xa8, 0x26,
        0xd2, 0x74, 0xe0, 0xa0, 0x0c, 0x2e, 0x2f, 0x74, 0x87, 0xce, 0x00, 0x89, 0x99, 0x1b, 0x0a,
        0x35, 0x4a, 0xc4, 0x96, 0x83, 0x7c, 0xa3, 0x74, 0xcc, 0x7d, 0xe3, 0x78, 0x20, 0x2a, 0x12,
        0x13, 0x19, 0x70, 0xa6, 0x3c, 0x7d, 0xc8, 0xd3, 0xed, 0x38, 0x84, 0xda, 0xbe, 0x0a, 0xbf,
        0xca, 0xc9, 0xa1, 0xf6, 0x6d, 0x89, 0x4b, 0xe4, 0x19, 0x36, 0xb7, 0x84, 0x66, 0x9d, 0x7d,
        0xb6, 0x72, 0x27, 0x7c, 0xef, 0x9f, 0x97, 0x99, 0x7c, 0x44, 0xf8, 0x3d, 0x83, 0xfd, 0x77,
        0xce, 0x4d, 0x8a, 0x04, 0x03, 0x28, 0x95, 0x46, 0xb2, 0xaa, 0x68, 0x54, 0x0b, 0xf1, 0x1d,
        0x65, 0x75, 0x10, 0xcd, 0x9b, 0x55, 0x02, 0x81, 0x80, 0x45, 0x9e, 0x30, 0x2b, 0x88, 0x6d,
        0xb9, 0xcb, 0xa5, 0x94, 0x65, 0x39, 0x0a, 0xf3, 0xba, 0x95, 0xd1, 0x19, 0x74, 0x67, 0x9a,
        0x3f, 0x83, 0xc1, 0xb0, 0xe6, 0x0b, 0x2a, 0x2b, 0x81, 0xa0, 0x1d, 0xb5, 0xec, 0x73, 0xef,
        0x0c, 0x6f, 0xcd, 0xb2, 0xaf, 0xdd, 0xd8, 0xd0, 0x6d, 0x31, 0x63, 0x4b, 0x39, 0xbf, 0x74,
        0xcf, 0x67, 0xb2, 0xf1, 0x9f, 0x7d, 0xdb, 0x8c, 0x44, 0x16, 0x89, 0xb2, 0xf4, 0x07, 0xe6,
        0x5f, 0xb9, 0x81, 0xab, 0x26, 0xd5, 0xcd, 0x83, 0x9e, 0xf8, 0x73, 0xcb, 0xce, 0x0b, 0x63,
        0xc7, 0x7e, 0xbd, 0x5a, 0xe3, 0x82, 0x63, 0x3d, 0x9d, 0x16, 0x0a, 0x78, 0x98, 0x1e, 0xfa,
        0x72, 0x41, 0x51, 0x56, 0xe8, 0x96, 0xc8, 0x27, 0x37, 0x47, 0x22, 0xa3, 0xf4, 0x7b, 0xd6,
        0xfd, 0xc2, 0x3b, 0x92, 0x00, 0x1a, 0xf6, 0x2c, 0x07, 0xbd, 0x01, 0x84, 0xcf, 0xe0, 0x05,
        0x83, 0xd9,
    ];

    /// Sign a synthetic JWT header+payload with the embedded test key.
    /// Returns (token_string, spki_der_of_public_key, JwtParts).
    pub(crate) fn build_signed_jwt(
        header_json: &str,
        payload_json: &str,
    ) -> (String, Vec<u8>, JwtParts) {
        use ring::{rand, rsa, signature};

        let key_pair = rsa::KeyPair::from_pkcs8(TEST_PRIVATE_KEY_PKCS8_DER)
            .expect("embedded test key must be a valid 2048-bit PKCS8 DER RSA private key");

        let header_b64 = crate::base64::encode(header_json.as_bytes());
        let payload_b64 = crate::base64::encode(payload_json.as_bytes());
        let signing_input = format!("{}.{}", header_b64, payload_b64);

        let rng = rand::SystemRandom::new();
        let mut sig = vec![0u8; key_pair.public().modulus_len()];
        key_pair
            .sign(
                &signature::RSA_PKCS1_SHA256,
                &rng,
                signing_input.as_bytes(),
                &mut sig,
            )
            .expect("signing must succeed with a valid 2048-bit key");

        let sig_b64 = crate::base64::encode(&sig);
        let token = format!("{}.{}.{}", header_b64, payload_b64, sig_b64);

        let rsa_pubkey_der: &[u8] = key_pair.public().as_ref();
        let spki = wrap_rsa_pubkey_as_spki(rsa_pubkey_der);

        let parts = JwtParts {
            header_b64,
            payload_b64,
            signature: sig,
        };

        (token, spki, parts)
    }

    /// Extract the RSA modulus (n) and public exponent (e) bytes from the embedded
    /// test PKCS8 key. Used by `crate::jwks::tests` for the round-trip test.
    /// Strips the DER leading-zero prefix so the bytes match what JWKS JSON encodes.
    pub(crate) fn embedded_test_modulus_exponent() -> (Vec<u8>, Vec<u8>) {
        let key_pair = ring::rsa::KeyPair::from_pkcs8(TEST_PRIVATE_KEY_PKCS8_DER)
            .expect("embedded test key must be valid");
        // `as_ref()` returns RSAPublicKey DER: SEQUENCE { INTEGER n, INTEGER e }
        let rsa_der = key_pair.public().as_ref();

        // Minimal DER length reader.
        fn read_len(buf: &[u8], pos: usize) -> (usize, usize) {
            let b = buf[pos];
            if b < 0x80 {
                return (b as usize, pos + 1);
            }
            let n = (b & 0x7f) as usize;
            let mut len = 0usize;
            for i in 0..n {
                len = (len << 8) | (buf[pos + 1 + i] as usize);
            }
            (len, pos + 1 + n)
        }

        // Parse outer SEQUENCE
        assert_eq!(rsa_der[0], 0x30, "expected SEQUENCE tag");
        let (_, seq_start) = read_len(rsa_der, 1);

        // Parse INTEGER n
        assert_eq!(rsa_der[seq_start], 0x02, "expected INTEGER tag for n");
        let (n_len, n_start) = read_len(rsa_der, seq_start + 1);
        let n_end = n_start + n_len;
        // Strip DER leading-zero sign prefix if present
        let n_bytes = if rsa_der[n_start] == 0x00 {
            rsa_der[n_start + 1..n_end].to_vec()
        } else {
            rsa_der[n_start..n_end].to_vec()
        };

        // Parse INTEGER e
        let e_pos = n_end;
        assert_eq!(rsa_der[e_pos], 0x02, "expected INTEGER tag for e");
        let (e_len, e_start) = read_len(rsa_der, e_pos + 1);
        let e_end = e_start + e_len;
        let e_bytes = if !rsa_der[e_start..e_end].is_empty() && rsa_der[e_start] == 0x00 {
            rsa_der[e_start + 1..e_end].to_vec()
        } else {
            rsa_der[e_start..e_end].to_vec()
        };

        (n_bytes, e_bytes)
    }

    /// Build a payload JSON with exp set far in the future.
    pub(crate) fn future_payload() -> String {
        r#"{"sub":"user-from-test","exp":18446744073709551615,"iss":"https://test.auth0.com/","aud":"test-app"}"#.to_string()
    }

    #[test]
    fn verify_accepts_valid_signature() {
        let header = r#"{"alg":"RS256","typ":"JWT"}"#;
        let (_token, spki, parts) = build_signed_jwt(header, &future_payload());
        assert_eq!(verify(&parts, &spki), Ok(()));
    }

    #[test]
    fn verify_rejects_tampered_signature() {
        let header = r#"{"alg":"RS256","typ":"JWT"}"#;
        let (_token, spki, mut parts) = build_signed_jwt(header, &future_payload());
        // Flip one byte of the signature — any byte works.
        parts.signature[0] ^= 0x01;
        assert_eq!(verify(&parts, &spki), Err(JwtError::SignatureInvalid));
    }

    #[test]
    fn verify_rejects_tampered_payload() {
        let header = r#"{"alg":"RS256","typ":"JWT"}"#;
        let (_token, spki, mut parts) = build_signed_jwt(header, &future_payload());
        // Replace payload_b64 with a different encoding — signature no longer covers it.
        parts.payload_b64 =
            crate::base64::encode(b"{\"sub\":\"evil\",\"exp\":99,\"iss\":\"x\",\"aud\":\"y\"}");
        assert_eq!(verify(&parts, &spki), Err(JwtError::SignatureInvalid));
    }

    #[test]
    fn verify_rejects_tampered_header() {
        let header = r#"{"alg":"RS256","typ":"JWT"}"#;
        let (_token, spki, mut parts) = build_signed_jwt(header, &future_payload());
        parts.header_b64 = crate::base64::encode(br#"{"alg":"none","typ":"JWT"}"#);
        assert_eq!(verify(&parts, &spki), Err(JwtError::SignatureInvalid));
    }

    #[test]
    fn verify_rejects_alg_none() {
        // A token that declares alg=none must be rejected before the signature
        // check — this is the algorithm confusion defense.
        let (_token, spki, _parts) =
            build_signed_jwt(r#"{"alg":"RS256","typ":"JWT"}"#, &future_payload());
        let parts = JwtParts {
            header_b64: crate::base64::encode(br#"{"alg":"none","typ":"JWT"}"#),
            payload_b64: crate::base64::encode(future_payload().as_bytes()),
            signature: vec![],
        };
        assert_eq!(verify(&parts, &spki), Err(JwtError::SignatureInvalid));
    }

    #[test]
    fn verify_rejects_alg_hs256() {
        // An HS256 token must be rejected; only RS256 is accepted.
        let (_token, spki, _parts) =
            build_signed_jwt(r#"{"alg":"RS256","typ":"JWT"}"#, &future_payload());
        let parts = JwtParts {
            header_b64: crate::base64::encode(br#"{"alg":"HS256","typ":"JWT"}"#),
            payload_b64: crate::base64::encode(future_payload().as_bytes()),
            signature: vec![0u8; 32],
        };
        assert_eq!(verify(&parts, &spki), Err(JwtError::SignatureInvalid));
    }

    #[test]
    fn verify_rejects_missing_alg_claim() {
        // A header with no alg field must return MissingClaim.
        let (_token, spki, _parts) =
            build_signed_jwt(r#"{"alg":"RS256","typ":"JWT"}"#, &future_payload());
        let parts = JwtParts {
            header_b64: crate::base64::encode(br#"{"typ":"JWT"}"#),
            payload_b64: crate::base64::encode(future_payload().as_bytes()),
            signature: vec![],
        };
        assert_eq!(verify(&parts, &spki), Err(JwtError::MissingClaim("alg")));
    }

    #[test]
    fn verify_rejects_malformed_spki() {
        let header = r#"{"alg":"RS256","typ":"JWT"}"#;
        let (_token, _spki, parts) = build_signed_jwt(header, &future_payload());
        let bad_spki = [0x00u8, 0x01, 0x02];
        assert_eq!(verify(&parts, &bad_spki), Err(JwtError::InvalidKey));
    }

    #[test]
    fn verify_rejects_corrupted_public_key() {
        let header = r#"{"alg":"RS256","typ":"JWT"}"#;
        let (_token, mut spki, parts) = build_signed_jwt(header, &future_payload());
        // Flip a byte inside the modulus region — past the SPKI header, inside the key data.
        // Byte ~50 is well inside the 2048-bit modulus (~256 bytes) regardless of exact SPKI offset.
        let flip_at = 50.min(spki.len() - 1);
        spki[flip_at] ^= 0xff;
        // Result is either SignatureInvalid (if ring parses it as a different valid-looking key)
        // or InvalidKey (if the byte corruption broke DER parsing).
        let result = verify(&parts, &spki);
        assert!(
            matches!(
                result,
                Err(JwtError::SignatureInvalid) | Err(JwtError::InvalidKey)
            ),
            "expected SignatureInvalid or InvalidKey, got {:?}",
            result
        );
    }

    #[test]
    fn end_to_end_parse_verify_extract() {
        let header = r#"{"alg":"RS256","typ":"JWT"}"#;
        let payload = future_payload();
        let (token, spki, _parts_unused) = build_signed_jwt(header, &payload);

        // Full pipeline: parse → verify → extract.
        let parts = parse(&token).expect("parse must succeed on self-signed token");
        verify(&parts, &spki).expect("verify must succeed on self-signed token");
        let claims =
            extract(&parts.payload_b64).expect("extract must succeed on future-exp payload");

        assert_eq!(claims.sub, "user-from-test");
        assert_eq!(claims.exp, u64::MAX);
        assert_eq!(claims.iss, "https://test.auth0.com/");
        assert_eq!(claims.aud, "test-app");
    }

    #[test]
    fn verify_signature_uses_only_domain_types() {
        // Compile-time proof that verify() only uses domain types in its API.
        let _: fn(&JwtParts, &[u8]) -> Result<(), JwtError> = verify;
    }

    // --- Plan 25-02: encode_der_length tests ---

    #[test]
    fn encode_der_length_short_form() {
        assert_eq!(encode_der_length(0), vec![0]);
        assert_eq!(encode_der_length(0x7F), vec![0x7F]);
    }

    #[test]
    fn encode_der_length_one_byte_long_form() {
        assert_eq!(encode_der_length(0x80), vec![0x81, 0x80]);
        assert_eq!(encode_der_length(0xFF), vec![0x81, 0xFF]);
    }

    #[test]
    fn encode_der_length_two_byte_long_form() {
        assert_eq!(encode_der_length(0x100), vec![0x82, 0x01, 0x00]);
        assert_eq!(encode_der_length(0xFFFF), vec![0x82, 0xFF, 0xFF]);
    }

    #[test]
    fn encode_der_length_three_byte_form() {
        assert_eq!(encode_der_length(0x10000), vec![0x83, 0x01, 0x00, 0x00]);
        assert_eq!(encode_der_length(0xFFFFFF), vec![0x83, 0xFF, 0xFF, 0xFF]);
    }
}
