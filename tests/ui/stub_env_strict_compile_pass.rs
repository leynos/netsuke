//! Control fixture proving the harness links `test_support` correctly.
//!
//! Were the `--extern` wiring broken, the compile-fail fixture would be
//! rejected for the wrong reason and its test would pass vacuously; this
//! fixture fails instead, naming the harness as the fault.

fn main() {
    let _ = test_support::locale_stubs::StubEnv::strict();
}
