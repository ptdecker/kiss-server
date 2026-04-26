//! JWKS fetch and SPKI DER extraction for Auth0 RS256 keys.
//!
//! At server startup `main.rs` calls [`fetch_spki_der`] with the JWKS URL from
//! `[server] jwks_url`. This module shells out to `/usr/bin/curl` (no async runtime,
//! no TLS client crate; D-03 prohibits both), parses the JSON response with the same
//! hand-rolled substring-scan idiom used in `src/jwt/mod.rs`, base64url-decodes the
//! `n` and `e` fields, assembles RSAPublicKey DER, and wraps it in an SPKI envelope.
//!
//! The returned SPKI bytes are exactly what `jwt::verify` expects — no further
//! conversion needed.
//!
//! # Limitations (acceptable for v1.6)
//!
//! - Only the FIRST key with `"alg":"RS256"` is extracted. Auth0 may serve multiple
//!   keys during rotation; matching by `kid` is deferred to a future phase.
//! - JWKS endpoint must be HTTPS reachable from the EC2 host with `curl` available
//!   at `/usr/bin/curl` (default on Amazon Linux 2).

use std::fmt;

/// All error cases surfaced when fetching or decoding a JWKS response.
#[derive(Debug)]
pub enum JwksError {
    /// `std::process::Command::new("/usr/bin/curl")` could not be launched.
    CurlSpawnFailed(String),
    /// curl ran but exited with a non-zero status (HTTP 4xx/5xx, timeout, DNS failure).
    CurlExitNonZero { code: Option<i32>, stderr: String },
    /// curl returned bytes that are not valid UTF-8.
    InvalidUtf8,
    /// JWKS JSON did not contain any object with `"alg":"RS256"`.
    KeyNotFound,
    /// `n` or `e` field present but base64url-decode failed.
    Base64DecodeError(&'static str),
    /// `n` or `e` field decoded to zero bytes — invalid RSA key.
    EmptyComponent(&'static str),
}

impl fmt::Display for JwksError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JwksError::CurlSpawnFailed(msg) => write!(
                f,
                "jwks: failed to launch /usr/bin/curl: {msg} \
                 (verify curl is installed at /usr/bin/curl on the host)"
            ),
            JwksError::CurlExitNonZero { code, stderr } => write!(
                f,
                "jwks: curl exited with status {:?}: {}",
                code,
                stderr.trim()
            ),
            JwksError::InvalidUtf8 => {
                write!(f, "jwks: response body is not valid UTF-8 (expected JSON)")
            }
            JwksError::KeyNotFound => write!(
                f,
                "jwks: response did not contain any key with \"alg\":\"RS256\""
            ),
            JwksError::Base64DecodeError(field) => {
                write!(f, "jwks: base64url decode failed for field '{field}'")
            }
            JwksError::EmptyComponent(field) => {
                write!(f, "jwks: field '{field}' decoded to zero bytes")
            }
        }
    }
}

impl std::error::Error for JwksError {}

/// Fetch the JWKS JSON from `url`, extract the first RS256 RSA key, and return
/// its SPKI DER bytes ready for `jwt::verify`.
pub fn fetch_spki_der(url: &str) -> Result<Vec<u8>, JwksError> {
    let json = fetch_jwks_json(url)?;
    jwks_json_to_spki_der(&json)
}

/// Convert an in-memory JWKS JSON string into SPKI DER. Used directly by tests
/// to avoid a real network call.
pub(crate) fn jwks_json_to_spki_der(json: &str) -> Result<Vec<u8>, JwksError> {
    let (n_b64, e_b64) = extract_rs256_key(json)?;
    let n_bytes = crate::base64::decode(n_b64).map_err(|_| JwksError::Base64DecodeError("n"))?;
    let e_bytes = crate::base64::decode(e_b64).map_err(|_| JwksError::Base64DecodeError("e"))?;
    if n_bytes.is_empty() {
        return Err(JwksError::EmptyComponent("n"));
    }
    if e_bytes.is_empty() {
        return Err(JwksError::EmptyComponent("e"));
    }
    let rsa_pubkey = rsa_pubkey_der(&n_bytes, &e_bytes);
    Ok(crate::jwt::wrap_rsa_pubkey_as_spki(&rsa_pubkey))
}

fn fetch_jwks_json(url: &str) -> Result<String, JwksError> {
    let output = std::process::Command::new("/usr/bin/curl")
        .args(["--silent", "--fail", "--max-time", "10", url])
        .output()
        .map_err(|e| JwksError::CurlSpawnFailed(e.to_string()))?;
    if !output.status.success() {
        return Err(JwksError::CurlExitNonZero {
            code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    String::from_utf8(output.stdout).map_err(|_| JwksError::InvalidUtf8)
}

/// Find the first `{ ..., "alg":"RS256", ... }` object in `json` and return
/// its `n` and `e` fields. Returns `KeyNotFound` if no such object exists.
fn extract_rs256_key(json: &str) -> Result<(&str, &str), JwksError> {
    // Walk the string looking for "alg":"RS256". For each hit, locate the
    // surrounding object's '{' and '}' and extract the n and e fields from
    // that range. Advance past incomplete objects so multi-key JWKS responses
    // (with some keys missing n/e) are handled gracefully.
    let mut search_from = 0;
    while let Some(rel_alg) = json[search_from..].find("\"alg\":\"RS256\"") {
        let alg_pos = search_from + rel_alg;
        // Find the opening '{' before alg_pos
        let Some(obj_start) = json[..alg_pos].rfind('{') else {
            break;
        };
        // Find the matching closing '}' after alg_pos (no nesting expected in JWK)
        let Some(obj_end_rel) = json[alg_pos..].find('}') else {
            break;
        };
        let obj_end = alg_pos + obj_end_rel;
        let obj = &json[obj_start..=obj_end];
        // Return this key if it has both n and e; otherwise advance past it
        if let (Some(n), Some(e)) = (extract_jwks_string(obj, "n"), extract_jwks_string(obj, "e")) {
            return Ok((n, e));
        }
        search_from = obj_end + 1;
    }
    Err(JwksError::KeyNotFound)
}

/// Find `"<key>":"<value>"` in `json` (no whitespace tolerance — same idiom
/// as `jwt::extract_string_claim`).
fn extract_jwks_string<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("\"{}\":\"", key);
    let start = json.find(&needle)? + needle.len();
    let end = json[start..].find('"')? + start;
    Some(&json[start..end])
}

/// Encode raw bytes as a DER INTEGER. Prepends 0x00 when the high bit of the
/// first byte is set (required for unsigned positive integers in DER).
fn der_integer(raw: &[u8]) -> Vec<u8> {
    let needs_leading_zero = !raw.is_empty() && raw[0] >= 0x80;
    let content_len = if needs_leading_zero {
        raw.len() + 1
    } else {
        raw.len()
    };
    let len_encoding = crate::jwt::encode_der_length(content_len);
    let mut out = Vec::with_capacity(1 + len_encoding.len() + content_len);
    out.push(0x02); // INTEGER tag
    out.extend_from_slice(&len_encoding);
    if needs_leading_zero {
        out.push(0x00);
    }
    out.extend_from_slice(raw);
    out
}

/// Assemble RFC 3447 RSAPublicKey DER: `SEQUENCE { INTEGER n, INTEGER e }`.
fn rsa_pubkey_der(n_bytes: &[u8], e_bytes: &[u8]) -> Vec<u8> {
    let n_der = der_integer(n_bytes);
    let e_der = der_integer(e_bytes);
    let inner_len = n_der.len() + e_der.len();
    let len_encoding = crate::jwt::encode_der_length(inner_len);
    let mut out = Vec::with_capacity(1 + len_encoding.len() + inner_len);
    out.push(0x30); // SEQUENCE tag
    out.extend_from_slice(&len_encoding);
    out.extend_from_slice(&n_der);
    out.extend_from_slice(&e_der);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- extract_jwks_string ---

    #[test]
    fn extract_jwks_string_finds_simple_value() {
        let json = r#"{"kty":"RSA","n":"abc","e":"AQAB"}"#;
        assert_eq!(extract_jwks_string(json, "n"), Some("abc"));
        assert_eq!(extract_jwks_string(json, "e"), Some("AQAB"));
        assert_eq!(extract_jwks_string(json, "kty"), Some("RSA"));
    }

    #[test]
    fn extract_jwks_string_returns_none_for_missing_key() {
        let json = r#"{"kty":"RSA"}"#;
        assert!(extract_jwks_string(json, "n").is_none());
    }

    // --- extract_rs256_key ---

    #[test]
    fn extract_rs256_key_finds_single_key() {
        let json = r#"{"keys":[{"kty":"RSA","alg":"RS256","n":"abc","e":"AQAB"}]}"#;
        let (n, e) = extract_rs256_key(json).expect("should find RS256 key");
        assert_eq!(n, "abc");
        assert_eq!(e, "AQAB");
    }

    #[test]
    fn extract_rs256_key_skips_non_rs256_and_finds_rs256() {
        let json = r#"{"keys":[{"kty":"RSA","alg":"HS256","n":"x","e":"y"},{"kty":"RSA","alg":"RS256","n":"good","e":"AQAB"}]}"#;
        let (n, e) = extract_rs256_key(json).expect("should find RS256 key");
        assert_eq!(n, "good");
        assert_eq!(e, "AQAB");
    }

    #[test]
    fn extract_rs256_key_returns_err_when_no_rs256_present() {
        let json = r#"{"keys":[{"kty":"RSA","alg":"HS256","n":"x","e":"y"}]}"#;
        assert!(matches!(
            extract_rs256_key(json),
            Err(JwksError::KeyNotFound)
        ));
    }

    // --- der_integer ---

    #[test]
    fn der_integer_no_leading_zero_when_high_bit_clear() {
        // 0x42 < 0x80 -> no prefix
        assert_eq!(der_integer(&[0x42]), vec![0x02, 0x01, 0x42]);
    }

    #[test]
    fn der_integer_leading_zero_when_high_bit_set() {
        // 0x80 >= 0x80 -> prefix with 0x00
        assert_eq!(der_integer(&[0x80]), vec![0x02, 0x02, 0x00, 0x80]);
        assert_eq!(der_integer(&[0xFF]), vec![0x02, 0x02, 0x00, 0xFF]);
    }

    #[test]
    fn der_integer_typical_rsa_modulus_first_byte() {
        // Auth0 RSA 2048 modulus typically starts with a high byte
        let modulus = [0xC1; 256];
        let encoded = der_integer(&modulus);
        assert_eq!(encoded[0], 0x02, "should start with INTEGER tag");
        // length is now 257 (256 + 1 leading zero) -> 2-byte length form
        assert_eq!(encoded[1], 0x82);
        assert_eq!(encoded[2], 0x01);
        assert_eq!(encoded[3], 0x01);
        assert_eq!(encoded[4], 0x00, "leading zero prefix expected");
        assert_eq!(encoded[5], 0xC1);
    }

    // --- rsa_pubkey_der ---

    #[test]
    fn rsa_pubkey_der_starts_with_sequence_tag() {
        let n = [0x42];
        let e = [0x01, 0x00, 0x01];
        let der = rsa_pubkey_der(&n, &e);
        assert_eq!(der[0], 0x30, "RSAPublicKey is a SEQUENCE");
    }

    // --- jwks_json_to_spki_der ---

    #[test]
    fn jwks_json_to_spki_der_returns_spki_envelope() {
        // Synthesized small key — n=0x42 (no leading zero), e=AQAB (=65537)
        let json = r#"{"keys":[{"kty":"RSA","alg":"RS256","n":"Qg","e":"AQAB"}]}"#;
        let spki = jwks_json_to_spki_der(json).expect("should produce SPKI");
        assert_eq!(spki[0], 0x30, "SPKI starts with outer SEQUENCE");
        assert!(
            spki.len() > 20,
            "SPKI envelope is at least the alg id + bit string"
        );
    }

    #[test]
    fn jwks_json_to_spki_der_rejects_missing_n() {
        let json = r#"{"keys":[{"kty":"RSA","alg":"RS256","e":"AQAB"}]}"#;
        assert!(matches!(
            jwks_json_to_spki_der(json),
            Err(JwksError::KeyNotFound)
        ));
    }

    #[test]
    fn jwks_json_to_spki_der_rejects_no_rs256_key() {
        let json = r#"{"keys":[{"kty":"RSA","alg":"HS256","n":"x","e":"y"}]}"#;
        assert!(matches!(
            jwks_json_to_spki_der(json),
            Err(JwksError::KeyNotFound)
        ));
    }

    // --- JwksError Display ---

    #[test]
    fn jwks_error_display_includes_helpful_text() {
        let err = JwksError::CurlSpawnFailed("no such file".to_string());
        let msg = err.to_string();
        assert!(
            msg.contains("/usr/bin/curl"),
            "spawn error should mention path: {msg}"
        );
        assert!(
            msg.contains("no such file"),
            "spawn error should include underlying msg: {msg}"
        );

        let err = JwksError::CurlExitNonZero {
            code: Some(22),
            stderr: "curl: (22) HTTP 404\n".to_string(),
        };
        assert!(
            err.to_string().contains("22"),
            "exit error should include code"
        );

        let err = JwksError::KeyNotFound;
        assert!(err.to_string().contains("RS256"));
    }

    // --- Round-trip test (Task 2) ---

    #[test]
    fn jwks_round_trip_with_embedded_test_key() {
        // Derive (n, e) from the embedded PKCS8 key
        let (n_bytes, e_bytes) = crate::jwt::tests::embedded_test_modulus_exponent();

        // Build a synthetic JWKS JSON wrapping those components
        let n_b64 = crate::base64::encode(&n_bytes);
        let e_b64 = crate::base64::encode(&e_bytes);
        let jwks_json = format!(
            r#"{{"keys":[{{"kty":"RSA","use":"sig","alg":"RS256","n":"{n_b64}","e":"{e_b64}"}}]}}"#
        );

        // Run our JWKS pipeline
        let spki =
            jwks_json_to_spki_der(&jwks_json).expect("synthetic JWKS should parse and assemble");

        // Sign a JWT with the same embedded key (use the existing test helper)
        let header = r#"{"alg":"RS256","typ":"JWT"}"#;
        let payload = crate::jwt::tests::future_payload();
        let (_token_str, _sig_bytes, parts) = crate::jwt::tests::build_signed_jwt(header, &payload);

        // Verify with the JWKS-derived SPKI — proves the full pipeline matches
        // the SPKI that wrap_rsa_pubkey_as_spki produces inside the jwt test suite
        crate::jwt::verify(&parts, &spki)
            .expect("JWKS-derived SPKI should verify a real signature");
    }
}
