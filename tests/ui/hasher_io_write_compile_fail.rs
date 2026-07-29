//! Compile-fail guard: 0.11 `Sha256` no longer implements `io::Write`.
//!
//! Before 0.11 `io::copy(&mut reader, &mut hasher)` compiled, because `Sha256`
//! implemented `std::io::Write`. In 0.11 that impl is gone, so this must fail
//! to compile. Netsuke feeds hashers through the `DigestWriter` adapter in
//! `netsuke::hasher` or an explicit buffered read loop instead.

use sha2::{Digest, Sha256};
use std::io;

fn main() {
    let mut hasher = Sha256::new();
    let data: &[u8] = b"abc";
    let mut reader = data;
    io::copy(&mut reader, &mut hasher).expect("copy into Sha256");
}
