//! Hash helpers for tests.
//!
//! These utilities expose deterministic SHA-256 encoding so behavioural and
//! integration tests can assert cache keys without duplicating hashing logic.
//!
//! # Examples
//!
//! ```rust
//! use test_support::hash::sha256_hex;
//!
//! let digest = sha256_hex(b"netsuke");
//! assert_eq!(digest.len(), 64);
//! ```
use sha2::{Digest, Sha256};

/// Compute the SHA-256 digest for `data` and return it as a lowercase hex
/// string.
pub fn sha256_hex(data: &[u8]) -> String {
    let digest = Sha256::digest(data);
    let mut key = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(&mut key, "{byte:02x}");
    }
    key
}
