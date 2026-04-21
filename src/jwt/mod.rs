//! JWT parsing, claims extraction, and RS256 signature verification.
//!
//! Staged API (per Phase 24 CONTEXT.md D-01/D-02):
//!   - `parse(token: &str) -> Result<JwtParts, JwtError>`
//!   - `verify(parts: &JwtParts, spki_der: &[u8]) -> Result<(), JwtError>`
//!   - `extract(payload_b64: &str) -> Result<JwtClaims, JwtError>`
//!
//! Implementation is delivered by Plans 02, 03, and 04.

// Intentional Wave 0 stub. Extended in subsequent plans.
