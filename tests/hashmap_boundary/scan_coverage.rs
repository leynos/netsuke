//! The scan sees the shapes the emitter could adopt, not only today's.
//!
//! The emitter-side test in the crate root can only report what the scan can
//! see, so a source shape the scan cannot reach is a hole no amount of correct
//! emitter code closes. These cases name a shape and assert a hit, so
//! narrowing the visitor fails here rather than silently widening what the
//! contract permits.
//!
//! Aliased spellings are shapes here too. The workspace reaches `HashMap`
//! through `IrHashMap` as well as by its own name, so a scan comparing
//! identifiers by equality covers only part of the tree it claims to.
//!
//! The `cfg` cases are the other half of the question. A scan that reported
//! every mention would be as wrong as one that missed aliases, and the
//! difference between `all(test, unix)` and `any(test, unix)` is not visible in
//! their spelling — it is in what the predicate requires. Those cases pin the
//! implication rule, so widening or narrowing it is a deliberate edit.
//!
//! The split from the crate root is a line-cap requirement with a real seam
//! behind it: the root states the rule and walks the tree, while this module
//! probes the reader that decides what the walk can find. Both files stay
//! inside the 400-line cap, and the scan's own unit of evidence — a syntax
//! tree — is what makes these cases expressible without a fixture tree.

use super::scan::{Hit, Kind, collect, parse};
use super::test_only::declares_test_only_module;
use rstest::rstest;

/// Every unordered type named by a constructor is found, including one
/// whose type is inferred and one written with a turbofish.
#[test]
fn an_expression_path_reaches_the_scan() {
    let source = "
            fn build() {
                let inferred = std::collections::HashMap::new();
                let qualified = std::collections::HashMap::<String, u8>::new();
                let _ = (inferred, qualified);
            }
        ";
    let parsed = parse(source).expect("the case is a valid source");
    let hits = collect("case.rs", &parsed);
    let constructors: Vec<&str> = hits
        .iter()
        .filter(|hit| hit.kind == Kind::Other)
        .map(|hit| hit.type_name.as_str())
        .collect();
    assert_eq!(
        constructors,
        ["HashMap", "HashMap"],
        "both the inferred and the turbofished constructor are uses"
    );
}

/// A collection named inside a test-only item is not a hit.
///
/// The counterpart to the case above: widening the visitor must not widen
/// what counts as reaching the emitter.
#[test]
fn a_test_only_expression_path_is_ignored() {
    let source = "
            #[cfg(test)]
            mod tests {
                fn build() {
                    let _ = std::collections::HashMap::new();
                }
            }
        ";
    let parsed = parse(source).expect("the case is a valid source");
    assert!(
        collect("case.rs", &parsed).is_empty(),
        "a release build never compiles the test module"
    );
}

/// A collection constructed inside a macro body is a hit.
///
/// `syn` hands a macro body to the visitor as tokens, so without a reader for
/// them `vec![HashMap::new()]` names no collection the scan can see. The
/// emitter writes `vec!` and `writeln!` throughout, so this is a shape it could
/// adopt tomorrow rather than a hypothetical.
#[test]
fn a_macro_body_reaches_the_scan() {
    let source = "fn build() { let seen = vec![std::collections::HashMap::new()]; }";
    let parsed = parse(source).expect("the case is a valid source");
    let hits = collect("case.rs", &parsed);
    assert_eq!(
        hits,
        [Hit {
            source: "case.rs".to_owned(),
            type_name: "HashMap".to_owned(),
            kind: Kind::Other,
        }],
        "the constructor sits inside the macro, not in the surrounding expression"
    );
}

/// A macro whose body has its own grammar is still read for the name.
///
/// `matches!` is not an expression list, so the body cannot be parsed as one.
/// Treating an unparseable body as empty would leave the most reachable macro
/// shape in the tree unread, which is why the fallback asks whether the tokens
/// spell the collection at all.
#[test]
fn an_unparseable_macro_body_is_still_read() {
    let source = "fn f(seen: &IrHashMap<String, u8>) { if matches!(seen, HashMap { .. }) {} }";
    let parsed = parse(source).expect("the case is a valid source");
    let hits = collect("case.rs", &parsed);
    assert!(
        hits.iter().any(|hit| hit.type_name == "HashMap"),
        "a body with a grammar of its own must not become a blind spot: {hits:?}"
    );
}

/// A field typed with an aliased collection is a hit for the collection.
///
/// The alias is how `BuildGraph` reaches `HashMap`, so a scan that reads only
/// unsuffixed spellings would leave the one production site the contract was
/// written for outside its own reach.
#[test]
fn an_aliased_field_reaches_the_scan() {
    let source = "struct S { seen: IrHashMap<String, u8> }";
    let parsed = parse(source).expect("the case is a valid source");
    let hits = collect("case.rs", &parsed);
    assert_eq!(
        hits,
        [Hit {
            source: "case.rs".to_owned(),
            type_name: "HashMap".to_owned(),
            kind: Kind::Field(Some("seen".to_owned())),
        }],
        "the hit names the collection as the exemption list spells it"
    );
}

/// An import of an aliased collection is a hit, however it is spelled.
///
/// The three shapes below are distinct arms of `UseTree`, and each is the only
/// spelling that reaches one of them: a plain path, a rename whose *source*
/// names the collection, and a rename whose *destination* does. Reading only
/// `ident` would miss the last, and reading only `rename` would miss the
/// second. The source strings differ; the expected finding does not.
#[rstest]
#[case::aliased("use crate::ir::graph::IrHashMap;\nstruct S { seen: IrHashMap<String, u8> }")]
#[case::renamed_alias("use crate::ir::graph::IrHashMap as Actions;")]
#[case::rename_onto_unordered("use crate::ir::graph::Actions as HashMap;")]
fn an_import_reaches_the_scan(#[case] source: &str) {
    let parsed = parse(source).expect("the case is a valid source");
    let imports: Vec<Hit> = collect("case.rs", &parsed)
        .into_iter()
        .filter(|hit| hit.kind == Kind::Import)
        .collect();
    assert_eq!(
        imports,
        [Hit {
            source: "case.rs".to_owned(),
            type_name: "HashMap".to_owned(),
            kind: Kind::Import,
        }],
        "whatever the arm, the import names the collection: {source}"
    );
}

/// Only a predicate that cannot hold outside `test` makes an item test-only.
///
/// `all(test, unix)` requires both, so a release build excludes it. `any(test,
/// unix)` requires either, so it compiles on Unix. `not(test)` compiles
/// everywhere except a test build. A rule reading the word `test` as text would
/// get two of these three backwards, and the mistake is invisible in the
/// spelling — which is why the predicate is parsed as a tree. `any(test)`
/// exercises the single-operand `any`, where "every disjunct implies `test`"
/// holds, so it is test-only even though `any` normally is not.
#[rstest]
#[case::all_with_unix("#[cfg(all(test, unix))]", true)]
#[case::bare_test("#[cfg(test)]", true)]
#[case::any_of_only_test("#[cfg(any(test))]", true)]
#[case::any_with_unix("#[cfg(any(test, unix))]", false)]
#[case::not_test("#[cfg(not(test))]", false)]
#[case::target_only("#[cfg(unix)]", false)]
fn a_cfg_predicate_implies_test_when_it_cannot_hold_elsewhere(
    #[case] attribute: &str,
    #[case] is_test_only: bool,
) {
    // The body must name a collection, or the case asserts nothing: an empty
    // function would be ignored whether or not the attribute stripped it.
    let source = format!("{attribute} fn helper() {{ let _ = std::collections::HashMap::new(); }}");
    let parsed = parse(&source).expect("the case is a valid source");
    assert_eq!(
        collect("case.rs", &parsed).is_empty(),
        is_test_only,
        "the finding must follow whether the predicate can hold outside a test build: {source}"
    );
}

/// A `#[cfg(test)] mod name;` declaration makes the named file test-only.
///
/// The child's contents carry no attribute of their own, so the declaration in
/// the parent is the only place the answer exists. This is the strip that keeps
/// `src/ninja_gen/property_tests/mod.rs` — whose `use` names both collections
/// without any attribute — out of a release artefact.
#[rstest]
#[case::declared_for_tests("#[cfg(test)] mod property_tests;", true)]
#[case::declared_always("mod property_tests;", false)]
#[case::other_module_for_tests("#[cfg(test)] mod tests;", false)]
fn a_test_only_module_declaration_is_reported(#[case] source: &str, #[case] expected: bool) {
    let parsed = parse(source).expect("the case is a valid source");
    assert_eq!(
        declares_test_only_module(&parsed, "property_tests"),
        expected,
        "the declaration names the child, and the attribute decides whether it compiles: {source}"
    );
}
