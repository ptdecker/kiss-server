//! RFC 4648 §5 base64url encode/decode. No padding. Alphabet uses '-' and '_'
//! in positions 62 and 63. JWT parts never include '=' padding; `decode`
//! rejects '=' and any other non-alphabet byte.

// See 24-RESEARCH.md Pitfall 6 — edition-2024 fires `dead_code` on `pub`
// items even when used only in `#[cfg(test)]`. `pub fn encode`/`pub fn decode`
// are not reached from `main.rs` until Phase 25 wires `AuthMiddleware` into
// dispatch. Suppress at the module level; Task 2 preserves this attribute
// when it overwrites the file with the real implementation.
#![allow(dead_code)]

pub fn encode(_bytes: &[u8]) -> String {
    todo!("implemented in Task 2")
}

pub fn decode(_s: &str) -> Result<Vec<u8>, &'static str> {
    todo!("implemented in Task 2")
}
