//! Guards for the `RustCrypto` 0.11 migration.
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
//! The runtime tests confirm the replacements produce correct digests, but
//! they cannot notice the pre-0.11 patterns becoming available again. A silent
//! downgrade to `sha2` 0.10 would not fail the ordinary build: 0.10's
//! `GenericArray` also derefs to `[u8]`, so `to_lower_hex` and `DigestWriter`
//! keep compiling. These assertions pin the absence of the two impls, which is
//! the signal that distinguishes the versions.
//!
//! Rust has no stable negative trait bound, so absence is observed with an
//! inherent-versus-trait probe: Rust resolves an inherent associated constant
//! ahead of a trait one, but only when the inherent impl's bound is satisfied.
//! `Probe::<T>::IMPLEMENTED` therefore reads `true` from the inherent impl when
//! `T` implements the trait, and falls back to the blanket trait impl's `false`
//! when it does not. Each assertion is paired with a positive control so the
//! probe cannot pass by reporting `false` for everything.
//!
//! This replaced a `trybuild` harness during the Polonius migration. Trybuild
//! always builds the host crate as a fixture dependency while discarding
//! workspace `build.rustflags`, so it rebuilt `netsuke` without
//! `-Zpolonius=next`; see the "Harness consequences" section of
//! `docs/polonius.md`. These probes need no subprocess, no scratch project, and
//! no toolchain-sensitive `.stderr` snapshot.

/// Define a compile-time probe module for one trait bound.
///
/// The generated `Probe::<T>::IMPLEMENTED` is `true` when `T` satisfies
/// `$bound` and `false` otherwise. Callers must bring `NotImplemented` into
/// scope for the fallback to resolve.
macro_rules! define_impl_probe {
    ($module:ident, $bound:path) => {
        mod $module {
            // Kept on one line: rustfmt re-indents a multi-line macro call inside an
            // attribute on every run, so a wrapped `concat!` here never reaches a fixed
            // point and `cargo fmt --check` can never pass.
            #![doc = concat!("Probes whether a type implements `", stringify!($bound), "`.")]
            #![doc = ""]
            #![doc = "`Probe::<T>::IMPLEMENTED` is `true` when that impl exists."]

            use core::marker::PhantomData;

            /// Carrier for the type under test.
            pub struct Probe<T>(PhantomData<T>);

            /// Fallback consulted when the inherent impl's bound is unmet.
            pub trait NotImplemented {
                /// Whether the probed trait is implemented.
                const IMPLEMENTED: bool = false;
            }

            impl<T> NotImplemented for Probe<T> {}

            impl<T: $bound> Probe<T> {
                /// Whether the probed trait is implemented.
                pub const IMPLEMENTED: bool = true;
            }
        }
    };
}

define_impl_probe!(lower_hex, core::fmt::LowerHex);
define_impl_probe!(io_write, std::io::Write);

/// The digest type returned by `Sha256::finalize()` and `Sha256::digest()`.
type Sha256Digest = sha2::digest::Output<sha2::Sha256>;

/// `format!("{:x}", digest)` must not compile, so the digest type must not
/// implement `LowerHex`.
///
/// The assertions sit in `const` blocks so the contract is enforced when the
/// test crate is compiled rather than when it runs: a breach fails the build
/// outright, which is the same signal the previous compile-fail fixtures gave.
#[test]
fn digest_output_does_not_implement_lower_hex() {
    use lower_hex::NotImplemented as _;

    const {
        assert!(
            lower_hex::Probe::<u8>::IMPLEMENTED,
            "probe is broken: u8 implements LowerHex"
        );
    }
    const {
        assert!(
            !lower_hex::Probe::<Sha256Digest>::IMPLEMENTED,
            concat!(
                "Sha256 digest output implements LowerHex, so a lower-hex format ",
                "specifier compiles again; this is how sha2 0.10 behaved, so check ",
                "the dependency has not been downgraded"
            )
        );
    }
}

/// `io::copy(reader, &mut hasher)` must not compile, so the hasher must not
/// implement `io::Write`.
///
/// Enforced at compile time, as above.
#[test]
fn hasher_does_not_implement_io_write() {
    use io_write::NotImplemented as _;

    const {
        assert!(
            io_write::Probe::<Vec<u8>>::IMPLEMENTED,
            "probe is broken: Vec<u8> implements io::Write"
        );
    }
    const {
        assert!(
            !io_write::Probe::<sha2::Sha256>::IMPLEMENTED,
            concat!(
                "Sha256 implements io::Write, so `io::copy` into a hasher compiles ",
                "again; this is how sha2 0.10 behaved, so check the dependency has ",
                "not been downgraded"
            )
        );
    }
}
