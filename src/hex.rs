//! Lowercase hexadecimal encoding for digest output.
//!
//! Digest rendering is deliberately explicit rather than relying on
//! `core::fmt::LowerHex`: the `RustCrypto` 0.11 family returns
//! `hybrid_array::Array` from `finalize()`, and that type does not implement
//! `LowerHex`. Routing every call site through this module keeps the rendered
//! form byte-identical across the workspace, which matters because action
//! digests and cache keys are persisted build identities.
//!
//! Each byte always renders as exactly two digits, including leading zeroes,
//! so the output is always twice the input length.
//!
//! # Examples
//!
//! ```
//! use netsuke::hex::to_lower_hex;
//!
//! assert_eq!(to_lower_hex(&[0x00, 0x0f, 0xa5]), "000fa5");
//! ```

/// Lowercase hexadecimal digits, indexed by nybble value.
const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";

/// Encode `bytes` as a lowercase hexadecimal string.
///
/// # Examples
///
/// ```
/// use netsuke::hex::to_lower_hex;
///
/// assert_eq!(to_lower_hex(b"\xde\xad\xbe\xef"), "deadbeef");
/// assert_eq!(to_lower_hex(&[]), "");
/// ```
#[must_use]
pub fn to_lower_hex(bytes: &[u8]) -> String {
    let mut hex = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        push_lower_hex_byte(&mut hex, byte);
    }
    hex
}

/// Append the two lowercase hexadecimal digits of `byte` to `output`.
///
/// Use this when only a prefix of a digest is needed and allocating the full
/// encoding would be wasteful.
///
/// # Examples
///
/// ```
/// use netsuke::hex::push_lower_hex_byte;
///
/// let mut out = String::new();
/// push_lower_hex_byte(&mut out, 0x0a);
/// assert_eq!(out, "0a");
/// ```
pub fn push_lower_hex_byte(output: &mut String, byte: u8) {
    // Indexing is provably in range for a nybble, but `get` keeps the helper
    // total rather than introducing a panic path in digest rendering.
    for nybble in [byte >> 4, byte & 0x0f] {
        if let Some(digit) = HEX_DIGITS.get(usize::from(nybble)).copied() {
            output.push(char::from(digit));
        }
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for lowercase hexadecimal encoding.
    //!
    //! These cover the full `u8` range rather than a handful of vectors,
    //! because a leading-zero or case regression here would silently change
    //! every persisted action digest and cache key.

    use super::{push_lower_hex_byte, to_lower_hex};
    use rstest::rstest;

    #[rstest]
    #[case(&[], "")]
    #[case(&[0x00], "00")]
    #[case(&[0xff], "ff")]
    #[case(&[0x0f, 0xf0], "0ff0")]
    #[case(b"\xde\xad\xbe\xef", "deadbeef")]
    fn encodes_known_vectors(#[case] bytes: &[u8], #[case] expected: &str) {
        assert_eq!(to_lower_hex(bytes), expected);
    }

    /// Every byte must render as exactly two lowercase digits that parse back
    /// to the original value, which catches leading-zero and case regressions
    /// that a handful of example vectors would miss.
    #[rstest]
    fn every_byte_round_trips() {
        for byte in u8::MIN..=u8::MAX {
            let encoded = to_lower_hex(&[byte]);
            assert_eq!(
                encoded.len(),
                2,
                "byte {byte:#04x} did not render two digits"
            );
            assert!(
                encoded
                    .chars()
                    .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)),
                "byte {byte:#04x} rendered non-lowercase-hex characters: {encoded}"
            );
            let decoded =
                u8::from_str_radix(&encoded, 16).expect("encoded byte should parse as hex");
            assert_eq!(decoded, byte);
        }
    }

    #[rstest]
    fn push_matches_whole_slice_encoding() {
        let bytes: Vec<u8> = (u8::MIN..=u8::MAX).collect();
        let mut pushed = String::new();
        for byte in &bytes {
            push_lower_hex_byte(&mut pushed, *byte);
        }
        assert_eq!(pushed, to_lower_hex(&bytes));
    }
}
