//! Property tests for reserved-name rejection in `register_manifest_vars`.
//!
//! The rstest cases in `tests/ast_tests/parsing.rs` pin the two reserved names
//! against a fixed manifest. These properties instead vary the whole `vars`
//! map, so the all-keys scan and the validate-before-register ordering are
//! exercised against arbitrary key sets and insertion orders rather than one
//! handwritten example.

use crate::manifest::{
    ManifestError, ManifestName, ManifestValue, RESERVED_VAR_NAMES, register_manifest_vars,
};
use minijinja::Environment;
use proptest::prelude::*;
use serde_json::{Map, Value, json};

/// Global names `MiniJinja` installs into every fresh environment.
///
/// A bare `Environment` already carries `debug`, `dict`, `namespace`, and
/// `range`, so "registered nothing" means "still exactly these", not "empty".
fn builtin_globals() -> Vec<String> {
    Environment::new()
        .globals()
        .map(|(name, _)| name.to_owned())
        .collect()
}

/// Sorted global names currently installed in `jinja`.
fn global_names(jinja: &Environment<'_>) -> Vec<String> {
    let mut names: Vec<String> = jinja.globals().map(|(name, _)| name.to_owned()).collect();
    names.sort();
    names
}

/// One of the reserved helper names.
fn reserved_name() -> impl Strategy<Value = &'static str> {
    proptest::sample::select(RESERVED_VAR_NAMES.to_vec())
}

/// Keys that are never reserved, kept short so shrinking stays readable.
///
/// Built-in global names are excluded as well: shadowing those is a separate
/// concern from the `env`/`glob` collision under test, and allowing them would
/// make "was this key added?" ambiguous against the baseline.
fn safe_key() -> impl Strategy<Value = String> {
    "[a-z][a-z_]{0,7}".prop_filter("must not collide with an existing global", |key| {
        !RESERVED_VAR_NAMES.contains(&key.as_str()) && !builtin_globals().contains(key)
    })
}

/// A small spread of value shapes, including nested ones.
fn var_value() -> impl Strategy<Value = Value> {
    prop_oneof![
        any::<bool>().prop_map(Value::from),
        any::<i32>().prop_map(Value::from),
        "[a-z ]{0,12}".prop_map(Value::from),
        proptest::collection::vec("[a-z]{1,4}", 0..3).prop_map(|items| json!(items)),
    ]
}

/// A `vars` map that contains no reserved name.
fn safe_vars() -> impl Strategy<Value = Map<String, Value>> {
    proptest::collection::hash_map(safe_key(), var_value(), 0..6)
        .prop_map(|entries| entries.into_iter().collect())
}

/// Build the `{ "vars": ... }` document `register_manifest_vars` reads.
fn doc_with_vars(vars: Map<String, Value>) -> ManifestValue {
    Value::Object(Map::from_iter([("vars".to_owned(), Value::Object(vars))]))
}

/// Register `vars` into a fresh environment, returning it alongside the result.
fn register(vars: Map<String, Value>) -> (Environment<'static>, Result<(), ManifestError>) {
    let mut jinja = Environment::new();
    let result = register_manifest_vars(
        &doc_with_vars(vars),
        &mut jinja,
        &ManifestName::new("Netsukefile"),
    );
    (jinja, result)
}

proptest! {
    /// Any map containing a reserved name is rejected, wherever it sits.
    #[test]
    fn reserved_key_anywhere_in_the_map_is_rejected(
        mut vars in safe_vars(),
        reserved in reserved_name(),
        value in var_value(),
    ) {
        vars.insert(reserved.to_owned(), value);

        let (_, result) = register(vars);

        prop_assert!(result.is_err(), "reserved key `{reserved}` should be rejected");
    }

    /// A rejected manifest registers nothing at all.
    ///
    /// This is the validate-before-register invariant: the scan runs to
    /// completion before the first `add_global`, so no sibling variable leaks
    /// into the environment when a later key turns out to be reserved.
    #[test]
    fn rejected_vars_leave_the_environment_untouched(
        mut vars in safe_vars(),
        reserved in reserved_name(),
        value in var_value(),
    ) {
        vars.insert(reserved.to_owned(), value);

        let (jinja, result) = register(vars);

        prop_assert!(result.is_err());
        let mut baseline = builtin_globals();
        baseline.sort();
        prop_assert_eq!(global_names(&jinja), baseline);
    }

    /// Every key of an accepted map becomes a global, with none dropped.
    #[test]
    fn accepted_vars_register_every_key(vars in safe_vars()) {
        let mut expected = builtin_globals();
        expected.extend(vars.keys().cloned());
        expected.sort();

        let (jinja, result) = register(vars);

        result.map_err(|e| TestCaseError::fail(e.to_string()))?;
        prop_assert_eq!(global_names(&jinja), expected);
    }
}
