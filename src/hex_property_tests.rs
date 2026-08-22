//! Property tests for lowercase hexadecimal encoding.
//!
//! The unit tests pin known vectors and sweep the whole `u8` range one byte at
//! a time. Neither reaches the invariants that only appear across a *slice*:
//! that the encoding is exactly two digits per byte however long the input, and
//! that it is a concatenation homomorphism, so encoding a slice equals encoding
//! its parts and joining them. A per-byte sweep cannot observe a bug that
//! reorders, drops, or duplicates bytes once more than one is present.
//!
//! These digests are persisted build identities — action hashes and fetch cache
//! keys — so a slice-wide encoding fault would silently invalidate caches
//! rather than fail loudly.

use proptest::prelude::*;

use super::{push_lower_hex_byte, to_lower_hex};

/// Decode a lowercase hexadecimal string back into bytes.
///
/// Returns `None` if the input is not an even-length run of lowercase
/// hexadecimal digits, so a malformed encoding fails the round trip rather
/// than being silently repaired.
fn decode_lower_hex(hex: &str) -> Option<Vec<u8>> {
    if !hex.is_ascii() || !hex.len().is_multiple_of(2) {
        return None;
    }
    let digits = hex.as_bytes();
    // The length is a multiple of two, checked above, so the remainder chunk
    // `as_chunks` returns alongside the pairs is always empty.
    digits
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            let text = std::str::from_utf8(pair).ok()?;
            u8::from_str_radix(text, 16).ok()
        })
        .collect()
}

proptest! {
    /// The encoding is always exactly two digits per input byte.
    #[test]
    fn encodes_two_digits_per_byte(bytes in prop::collection::vec(any::<u8>(), 0..512)) {
        prop_assert_eq!(to_lower_hex(&bytes).len(), bytes.len() * 2);
    }

    /// Every character is a lowercase hexadecimal digit.
    #[test]
    fn emits_only_lowercase_hex_digits(bytes in prop::collection::vec(any::<u8>(), 0..512)) {
        let encoded = to_lower_hex(&bytes);
        prop_assert!(
            encoded.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)),
            "unexpected characters in {encoded}"
        );
    }

    /// Encoding round-trips: decoding the output recovers the input exactly.
    ///
    /// This is the property that catches a reordering or off-by-one fault,
    /// which a length check alone would pass.
    #[test]
    fn round_trips_through_decoding(bytes in prop::collection::vec(any::<u8>(), 0..512)) {
        let decoded = decode_lower_hex(&to_lower_hex(&bytes));
        prop_assert_eq!(decoded.as_deref(), Some(bytes.as_slice()));
    }

    /// Encoding distributes over concatenation.
    ///
    /// `to_lower_hex(a ++ b) == to_lower_hex(a) ++ to_lower_hex(b)` means each
    /// byte is encoded independently of its neighbours and of its position, so
    /// no state leaks between iterations.
    #[test]
    fn distributes_over_concatenation(
        left in prop::collection::vec(any::<u8>(), 0..256),
        right in prop::collection::vec(any::<u8>(), 0..256),
    ) {
        let mut joined = left.clone();
        joined.extend_from_slice(&right);

        let expected = to_lower_hex(&left) + &to_lower_hex(&right);
        prop_assert_eq!(to_lower_hex(&joined), expected);
    }

    /// The whole-slice encoder agrees with the per-byte one.
    ///
    /// `push_lower_hex_byte` renders truncated digests in `manifest::expand`,
    /// so the two must not drift apart.
    #[test]
    fn agrees_with_the_per_byte_encoder(bytes in prop::collection::vec(any::<u8>(), 0..512)) {
        let mut pushed = String::new();
        for byte in &bytes {
            push_lower_hex_byte(&mut pushed, *byte);
        }
        prop_assert_eq!(pushed, to_lower_hex(&bytes));
    }
}
