//! Tests for `StubEnv`'s strictness about undeclared reads.
//!
//! Without these, relaxing or deleting the assertion would leave every other
//! test passing while restoring exactly the permissive behaviour the stub was
//! made strict to remove.

use netsuke::locale_resolution::LocaleEnvProvider;
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

mod properties {
    //! Property coverage for the builder's declaration semantics.
    //!
    //! The fixed cases above check single interleavings of `with_var` and
    //! `allowing`; this states the invariant they are instances of — the last
    //! declaration for a key wins — over arbitrary declaration sequences.

    use super::{LocaleEnvProvider, StubEnv};
    use proptest::collection::vec;
    use proptest::prelude::*;
    use std::cell::Cell;
    use std::collections::HashMap;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::sync::Once;

    #[derive(Debug, Clone)]
    enum Declaration {
        Allow(String),
        Set(String, String),
    }

    /// Three keys only, so generated sequences redeclare the same key often
    /// enough for ordering to matter; a wide key space would almost never
    /// produce the collisions the invariant is about.
    fn declaration() -> impl Strategy<Value = Declaration> {
        prop_oneof![
            "[ABC]".prop_map(Declaration::Allow),
            ("[ABC]", "[a-z]{1,4}").prop_map(|(key, value)| Declaration::Set(key, value)),
        ]
    }

    thread_local! {
        static SILENCED: Cell<bool> = const { Cell::new(false) };
    }

    /// Install the gated hook exactly once, wrapping whatever hook was
    /// current when the first probe ran.
    fn install_gated_hook() {
        let prior = std::panic::take_hook();
        let gated = move |info: &std::panic::PanicHookInfo<'_>| {
            if SILENCED.with(Cell::get) {
                return;
            }
            prior(info);
        };
        std::panic::set_hook(Box::new(gated));
    }

    /// Run `probe` with the default panic hook silenced for this thread only.
    ///
    /// Each undeclared read otherwise prints its full panic message, and 256
    /// cases times three keys of that buries any genuine failure output. The
    /// hook is process-wide, so instead of swapping it around each probe —
    /// which under the threaded in-process coverage runner could eat another
    /// test's panic or race the restore — a wrapper is installed once and
    /// consults a thread-local flag, leaving every other thread's panics on
    /// the prior hook.
    fn silenced<T>(probe: impl FnOnce() -> T) -> T {
        static INSTALL: Once = Once::new();
        INSTALL.call_once(install_gated_hook);

        SILENCED.with(|flag| flag.set(true));
        let result = probe();
        SILENCED.with(|flag| flag.set(false));
        result
    }

    proptest! {
        /// Every key answers per its last declaration; undeclared keys panic.
        ///
        /// The model is a plain last-write-wins map, independent of the
        /// stub's split `values`/`allowed` representation, so a bookkeeping
        /// slip between the two collections fails here rather than agreeing
        /// with itself.
        #[test]
        fn the_last_declaration_wins_over_any_sequence(
            declarations in vec(declaration(), 0..8)
        ) {
            let mut model: HashMap<String, Option<String>> = HashMap::new();
            let mut stub = StubEnv::strict();
            for declaration in &declarations {
                match declaration {
                    Declaration::Allow(key) => {
                        model.insert(key.clone(), None);
                        stub = stub.allowing(key.clone());
                    }
                    Declaration::Set(key, value) => {
                        model.insert(key.clone(), Some(value.clone()));
                        stub = stub.with_var(key.clone(), value.clone());
                    }
                }
            }
            for key in ["A", "B", "C"] {
                if let Some(expected) = model.get(key) {
                    prop_assert_eq!(stub.var(key), expected.clone());
                } else {
                    let read = silenced(|| catch_unwind(AssertUnwindSafe(|| stub.var(key))));
                    prop_assert!(read.is_err(), "undeclared {} should panic", key);
                }
            }
        }
    }
}
