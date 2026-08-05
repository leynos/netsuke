//! `StubEnv::default()` must not compile.
//!
//! `Default` on a strict stub would mean "deny every read", so the common
//! "no locale set" case would compile and then panic at run time; the builder
//! constructors keep that intent explicit at compile time.

fn main() {
    let _ = test_support::locale_stubs::StubEnv::default();
}
