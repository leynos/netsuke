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
use netsuke::hex::to_lower_hex;
use sha2::{Digest, Sha256};

/// Compute the SHA-256 digest for `data` and return it as a lowercase hex
/// string.
///
/// Encoding is delegated to `netsuke::hex` so test expectations cannot drift
/// from the rendering the production digest paths use.
#[must_use]
pub fn sha256_hex(data: &[u8]) -> String {
    to_lower_hex(&Sha256::digest(data))
}
