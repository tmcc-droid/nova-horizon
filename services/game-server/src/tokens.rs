//! Opaque refresh / connect tickets.

use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};

/// Cryptographically random ticket (hex). Shown once to client as refresh_token / connect_ticket.
pub fn mint_opaque_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

pub fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

pub fn tokens_match(raw: &str, stored_hash: &str) -> bool {
    hash_token(raw) == stored_hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_roundtrip() {
        let t = mint_opaque_token();
        let h = hash_token(&t);
        assert!(tokens_match(&t, &h));
        assert!(!tokens_match("nope", &h));
    }
}
