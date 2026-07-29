//! Compile-fail guards for the `RustCrypto` 0.11 migration.
//!
//! The 0.11 family made two source-breaking changes that this codebase works
//! around:
//!
//! - `Sha256::finalize()` / `Sha256::digest()` return
//!   `hybrid_array::Array<u8, _>`, which does not implement
//!   [`core::fmt::LowerHex`], so `format!("{:x}", digest)` no longer compiles.
//!   Digests render through `netsuke::hex::to_lower_hex` instead.
//! - `Sha256` no longer implements [`std::io::Write`], so
//!   `io::copy(reader, &mut hasher)` no longer compiles. Hashers are fed
//!   through the `DigestWriter` adapter in `netsuke::hasher` or an explicit
//!   buffered read loop instead.
//!
//! Runtime tests confirm the replacements produce the right digests, but they
//! cannot notice the pre-0.11 patterns becoming available again — for example
//! if `sha2` were downgraded to 0.10. These fixtures pin the removals: should
//! either start compiling, this test fails and names the regression.
//!
//! The `.stderr` fixtures record diagnostics from the toolchain pinned in
//! `rust-toolchain.toml`. A toolchain bump can reword them without any real
//! regression; re-bless with `TRYBUILD=overwrite cargo test --test
//! sha2_migration_ui_tests` and check the diff still shows the same unsatisfied
//! trait bound.

/// The pre-0.11 digest-formatting and hasher-`io::Write` patterns must not
/// compile.
#[test]
fn pre_0_11_digest_patterns_fail_to_compile() {
    let cases = trybuild::TestCases::new();
    cases.compile_fail("tests/ui/digest_lower_hex_compile_fail.rs");
    cases.compile_fail("tests/ui/hasher_io_write_compile_fail.rs");
}
