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
//! The split from the crate root is a line-cap requirement with a real seam
//! behind it: the root states the rule and walks the tree, while this module
//! probes the reader that decides what the walk can find. Both files stay
//! inside the 400-line cap, and the scan's own unit of evidence — a syntax
//! tree — is what makes these cases expressible without a fixture tree.

use super::scan::{Hit, Kind, collect, parse};

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

/// An import of an aliased collection is a hit too.
///
/// A `use` reaches the emitter's bytes on its own, so the alias has to be
/// recognized on the import path and not only where the field is declared.
#[test]
fn an_aliased_import_reaches_the_scan() {
    let source = "use crate::ir::graph::IrHashMap;\nstruct S { seen: IrHashMap<String, u8> }";
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
        "an alias ending in HashMap is an unordered collection"
    );
}

/// A renamed import of an aliased collection is a hit on the original.
///
/// `use …::IrHashMap as Actions` writes the alias in the file's own code, but
/// the `use` still names the collection, and `rename` holds only the alias, so
/// this shape is visible solely if the arm reads `ident`. It is the shape the
/// plain-import case above cannot reach, which is why it carries its own test
/// rather than being folded into that one.
#[test]
fn a_renamed_aliased_import_reaches_the_scan() {
    let source = "use crate::ir::graph::IrHashMap as Actions;";
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
        "the alias is what the file writes, but the tree names the collection"
    );
}

/// A rename to an unordered spelling is a hit for the collection too.
///
/// The mirror of the case above: `use …::Whatever as HashMap` writes the
/// collection's own name into the file, so the *destination* of the rename is
/// what the file subsequently iterates. Reading only `ident` would miss it.
#[test]
fn a_rename_onto_an_unordered_name_reaches_the_scan() {
    let source = "use crate::ir::graph::Actions as HashMap;";
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
        "a rename onto an unordered name puts that collection in the file"
    );
}
