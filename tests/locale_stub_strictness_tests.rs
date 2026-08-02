//! Tests for `StubEnv`'s strictness about undeclared reads.
//!
//! Without these, relaxing or deleting the assertion would leave every other
//! test passing while restoring exactly the permissive behaviour the stub was
//! made strict to remove.

use netsuke::locale_resolution::EnvProvider;
use test_support::locale_stubs::StubEnv;

#[test]
#[should_panic(expected = "which the test did not declare")]
fn undeclared_read_panics() {
    StubEnv::strict().var("SOME_UNDECLARED_VARIABLE");
}

#[test]
#[should_panic(expected = "SOME_OTHER_VARIABLE")]
fn the_panic_names_the_offending_key() {
    StubEnv::with_locale("es-ES").var("SOME_OTHER_VARIABLE");
}

#[test]
fn declared_reads_do_not_panic() {
    assert_eq!(
        StubEnv::with_locale("es-ES")
            .var("NETSUKE_LOCALE")
            .as_deref(),
        Some("es-ES")
    );
    assert_eq!(StubEnv::without_locale().var("NETSUKE_LOCALE"), None);
}

/// The most recent declaration for a key wins, in either order.
///
/// Were `allowing` merely to append to the permitted list, the second case
/// would read as declaring the key unset while still answering `Some("set")`.
#[test]
fn the_last_declaration_for_a_key_wins() {
    let value_then_unset = StubEnv::strict().with_var("X", "set").allowing("X");
    assert_eq!(value_then_unset.var("X"), None, "allowing should clear");

    let unset_then_value = StubEnv::strict().allowing("X").with_var("X", "set");
    assert_eq!(
        unset_then_value.var("X").as_deref(),
        Some("set"),
        "with_var should override"
    );
}

/// Declaring a key twice must not make the stub answer differently.
#[test]
fn repeated_declaration_is_idempotent() {
    let env = StubEnv::strict()
        .with_var("X", "first")
        .with_var("X", "second");
    assert_eq!(env.var("X").as_deref(), Some("second"));
}
