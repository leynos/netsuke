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

#[cfg(test)]
mod tests {
    //! Tests pinning `sha256_hex` to published SHA-256 test vectors.
    //!
    //! This helper predicts fetch cache keys for behavioural tests, so it is
    //! the yardstick those tests measure production output against. Checking
    //! it against known vectors rather than against another call of itself
    //! means a hashing or encoding regression cannot hide by moving both the
    //! expectation and the result together.

    use super::sha256_hex;

    #[test]
    fn matches_the_known_vector_for_abc() {
        assert_eq!(
            sha256_hex(b"abc"),
            concat!(
                "ba7816bf8f01cfea414140de5dae2223",
                "b00361a396177a9cb410ff61f20015ad",
            ),
        );
    }

    #[test]
    fn matches_the_known_vector_for_empty_input() {
        assert_eq!(
            sha256_hex(b""),
            concat!(
                "e3b0c44298fc1c149afbf4c8996fb924",
                "27ae41e4649b934ca495991b7852b855",
            ),
        );
    }
}
