//! Compile-time UI tests for `Captured::state()` lifetime correctness.
//!
//! `MacroStateGuard` (in `src/manifest/jinja_macros/cache.rs`) relies on the
//! borrow checker rejecting any programme that lets a `&State` outlive the
//! owning `Captured`. These trybuild cases pin that guarantee: the dangling
//! case must fail to compile, and the well-scoped case must compile.

#[test]
fn captured_state_lifetime_tests() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/captured_state_outlives_captured.rs");
    t.pass("tests/ui/captured_state_within_scope.rs");
}
