//! Property coverage for `CommandEnv` and `PATH` composition.
//!
//! Split from `env_path_tests.rs` to keep that file within the 400-line
//! ceiling; included from there via `#[path]`, so the fixed cases and the
//! invariants they are instances of still ship as one test binary. These
//! properties cover inputs nobody would write down: arbitrary entry lists
//! including empty entries, and arbitrary override sequences with repeated
//! keys.

//! Property coverage for `CommandEnv` and `PATH` composition.
//!
//! The fixed cases above name specific behaviours; these state the
//! invariants they are instances of, over inputs nobody would write down:
//! arbitrary entry lists including empty entries, and arbitrary override
//! sequences with repeated keys.

use netsuke::runner::CommandEnv;
use proptest::collection::vec;
use proptest::prelude::*;
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use test_support::env::prepend_path_value;

/// One `PATH` entry: separator-free on every platform, possibly empty.
///
/// Empty entries are generated deliberately — they are meaningful on Unix
/// (the current directory) and must survive composition when they sit
/// inside a non-empty value.
fn entry() -> impl Strategy<Value = String> {
    prop_oneof![
        1 => Just(String::new()),
        4 => "[A-Za-z0-9_./-]{1,8}",
    ]
}

proptest! {
    /// Composition prepends `dir` and preserves the existing entries and
    /// their order exactly; an absent value yields only `dir`, and — by
    /// `prepend_path_value`'s contract — so does a wholly empty one.
    ///
    /// The expectation splits the *input* value rather than echoing the
    /// generated list, because `join_paths` cannot distinguish an empty
    /// list from one empty entry — that ambiguity belongs to the `PATH`
    /// representation, and the helper's contract is over the value it
    /// receives. A dropped or reordered entry still cannot agree with
    /// itself, since input and output are split independently.
    #[test]
    fn composition_prepends_and_preserves_order(
        existing in prop::option::of(vec(entry(), 0..6)),
        dir in "[A-Za-z0-9_./-]{1,8}",
    ) {
        let joined = existing
            .as_ref()
            .map(|parts| std::env::join_paths(parts).expect("separator-free entries join"));
        let composed = prepend_path_value(joined.as_deref(), Path::new(&dir))
            .expect("separator-free inputs compose");

        let mut expected = vec![PathBuf::from(&dir)];
        if let Some(value) = joined.as_deref().filter(|value| !value.is_empty()) {
            expected.extend(std::env::split_paths(value));
        }
        let found: Vec<PathBuf> = std::env::split_paths(&composed).collect();
        prop_assert_eq!(found, expected);
    }

    /// `get` answers per the last override for each key, and `is_empty`
    /// holds exactly when no override was ever set.
    ///
    /// The model is a plain last-write-wins map, independent of the
    /// implementation's in-place-update vector, so a bookkeeping slip
    /// between lookup and storage fails here.
    #[test]
    fn overrides_resolve_to_their_last_declaration(
        ops in vec(("[AB]", "[a-z]{0,4}"), 0..8)
    ) {
        let mut model: HashMap<String, String> = HashMap::new();
        let mut env = CommandEnv::inherit();
        for (key, value) in &ops {
            model.insert(key.clone(), value.clone());
            env = env.with_var(key, value);
        }
        prop_assert_eq!(env.is_empty(), model.is_empty());
        for key in ["A", "B", "C"] {
            let expected = model.get(key).map(|value| OsString::from(value.clone()));
            prop_assert_eq!(
                env.get(key),
                expected.as_deref().map(OsStr::new),
                "key {}", key
            );
        }
    }
}
