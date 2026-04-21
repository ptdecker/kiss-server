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
//! # Dead-code suppression
//!
//! Under Rust edition 2024, `dead_code` warnings fire on `pub` items that are
//! not yet reachable from `main.rs`. Phase 25 will wire `AuthMiddleware` into
//! the dispatch chain at which point this attribute can be removed.
#![allow(dead_code)]

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
    let aud = extract_string_claim(json, "aud")
        .ok_or(JwtError::MissingClaim("aud"))?
        .to_string();

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
fn extract_string_claim<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("\"{}\":\"", key);
    let start = json.find(&needle)? + needle.len();
    let end = json[start..].find('"')? + start;
    Some(&json[start..end])
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

// verify() is implemented in Plan 04.

#[cfg(test)]
mod tests {
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
}
