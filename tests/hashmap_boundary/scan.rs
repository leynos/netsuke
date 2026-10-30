//! The scan behind the emitter's unordered-collection boundary contract.
//!
//! Split from the crate root because the contract's *rule* and its *reading*
//! are two jobs: the root states what the emitter may hold, and this module
//! answers what a source actually holds. The split is also what keeps both
//! files inside the repository's line cap.
//!
//! The unit of evidence is a syntax tree rather than source text. A collection
//! named in a doc comment, or quoted in a string literal, is not a type; a walk
//! over the parse tree gets that distinction for free, while a text scan has to
//! restate it as a masking rule and can always get one spelling wrong. The
//! crate therefore depends on `syn`, which the workspace already carries for
//! its resolver boundary contract.
//!
//! Test-only items are removed by the `cfg` predicate, not by module name. The
//! emitter's own property suites live in `#[cfg(test)]` modules and hold both
//! collections; a release build never compiles them, so their contents cannot
//! reach an emitted byte. An allowlist of paths would say the same thing less
//! precisely, and would excuse a production file that later took a listed name.

use super::macro_body;
use super::test_only::is_test_only;
use syn::{
    Field, File, Item, Path, Type, UseTree,
    visit::{self, Visit},
};

/// Collection spellings that carry no order of their own.
///
/// The ordered counterparts — `BTreeMap`, `BTreeSet`, `IndexMap`, and `Vec` —
/// are deliberately absent. They already impose an order, and flagging them
/// would push a future author towards a hash collection plus a sort call rather
/// than towards the collection that sorts itself.
pub(super) const UNORDERED_TYPES: [&str; 2] = ["HashMap", "HashSet"];

/// One place a compiled source names an unordered collection.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Hit {
    /// Workspace-relative path of the source the name sits in.
    pub(super) source: String,
    /// The type name as written, one of [`UNORDERED_TYPES`].
    pub(super) type_name: String,
    /// What the name was written for.
    pub(super) kind: Kind,
}

/// What a hit was written for, which is what the exemption list names.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Kind {
    /// A struct field's declared type, with the field's name where it has one.
    Field(Option<String>),
    /// An import of the type.
    Import,
    /// Any other type position, such as a local binding's annotation.
    Other,
}

impl Kind {
    /// The field name the exemption list can name, when this hit is a field.
    pub(super) const fn field(&self) -> Option<&str> {
        match self {
            Self::Field(Some(name)) => Some(name.as_str()),
            Self::Field(None) | Self::Import | Self::Other => None,
        }
    }
}

/// Parse one source into the tree the scan reads.
///
/// Parsing lives here rather than at the call site so the contract test never
/// handles a syntax tree directly: it supplies text and receives findings.
///
/// # Errors
///
/// Returns the parser's error when `text` is not a Rust source file.
pub(super) fn parse(text: &str) -> syn::Result<File> {
    syn::parse_file(text)
}

/// Record every unordered collection `file` names outside its test-only items.
///
/// Imports are recorded because a `use` on its own can be iterated later in a
/// function body; fields are recorded because a long-lived collection lives
/// there, and the field's name is what an exemption can name; every other type
/// position is recorded so a local `let` binding annotation or a type alias is
/// not a blind spot.
///
pub(super) fn collect(path: &str, file: &File) -> Vec<Hit> {
    let mut scan = Scan {
        source: path.to_owned(),
        hits: Vec::new(),
    };
    scan.visit_file(file);
    scan.hits
}

/// The visitor that produces [`Hit`]s.
struct Scan {
    /// Workspace-relative path of the file being visited.
    source: String,
    /// Hits recorded so far, in visit order.
    hits: Vec<Hit>,
}

impl Scan {
    /// Record one hit, attributing it to the file being visited.
    fn record(&mut self, kind: Kind, type_name: &'static str) {
        self.hits.push(Hit {
            source: self.source.clone(),
            type_name: type_name.to_owned(),
            kind,
        });
    }
}

impl<'ast> Visit<'ast> for Scan {
    /// Visit `item` unless it is compiled only for tests.
    ///
    /// This is the single place the attribute strip happens, so every item
    /// position the grammar admits — including an inline `mod tests { ... }`
    /// and a `use` inside a function body — is covered by one decision rather
    /// than by a descent that would have to repeat it. A file-backed module
    /// whole-file test-only-ness is decided by the walk before the text ever
    /// reaches here, from the `mod` declaration in the parent.
    fn visit_item(&mut self, item: &'ast Item) {
        if !is_test_only(item) {
            visit::visit_item(self, item);
        }
    }

    /// Record an import that reaches an unordered collection.
    fn visit_item_use(&mut self, node: &'ast syn::ItemUse) {
        for type_name in UNORDERED_TYPES {
            if names_segment(&node.tree, type_name) {
                self.record(Kind::Import, type_name);
            }
        }
    }

    /// Record a field whose declared type is unordered; otherwise descend.
    ///
    /// A classified field is not descended into, because its type has just been
    /// recorded and visiting it again would record the same name under a shape
    /// the exemption list cannot name. A field of some other type is descended
    /// normally, so a `Vec<HashMap<..>>` field is still found — one level in,
    /// as [`Kind::Other`].
    fn visit_field(&mut self, node: &'ast Field) {
        if let Some(type_name) = unordered_name(&node.ty) {
            let field = node.ident.as_ref().map(ToString::to_string);
            self.record(Kind::Field(field), type_name);
            return;
        }
        visit::visit_field(self, node);
    }

    /// Record any other type position naming an unordered collection.
    fn visit_type_path(&mut self, node: &'ast syn::TypePath) {
        if let Some(type_name) = path_unordered_name(&node.path) {
            self.record(Kind::Other, type_name);
        }
        visit::visit_type_path(self, node);
    }

    /// Record an expression path naming an unordered collection.
    ///
    /// A `TypePath` only covers annotations and aliases, so `let seen =
    /// HashMap::new()` — where the type is inferred from the constructor — is
    /// invisible to the rest of this visitor. It is a real use: the binding is
    /// a collection and can be iterated.
    ///
    /// The matching rule differs from a type path's. An expression path names
    /// a value's *way in*: `HashMap::new()` ends in `new`, and
    /// `std::collections::HashMap::<K, V>::new()` ends the same way, so
    /// reading the final segment alone would never see either. Any segment
    /// naming an unordered collection is therefore a hit. In a value position
    /// such a segment can only be the collection itself or a module named
    /// after it, and flagging the latter is a finding a human can dismiss,
    /// where missing the former is a hole in the contract.
    fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
        if let Some(type_name) = any_segment_unordered(&node.path) {
            self.record(Kind::Other, type_name);
        }
        visit::visit_expr_path(self, node);
    }

    /// Read a macro body, which the default descent sees only as tokens.
    ///
    /// `vec![HashMap::new()]` is a real use of the collection: the `vec!` body
    /// holds an expression, and nothing in the grammar's item or expression
    /// hooks reaches it. A body that parses as an expression list is therefore
    /// descended into like any other code. The rest have a grammar of their own,
    /// so each is asked whether it spells a matched name at all; the same
    /// err-towards-reporting choice [`any_segment_unordered`] makes for a bare
    /// value path applies here.
    fn visit_macro(&mut self, node: &'ast syn::Macro) {
        if let Some(arguments) = macro_body::arguments(node) {
            for argument in &arguments {
                self.visit_expr(argument);
            }
        } else {
            let spelled = UNORDERED_TYPES
                .iter()
                .copied()
                .filter(|type_name| macro_body::spells(node, type_name));
            for type_name in spelled {
                self.record(Kind::Other, type_name);
            }
        }
    }
}

/// The unordered type a field or binding declares, if it declares one.
fn unordered_name(ty: &Type) -> Option<&'static str> {
    let Type::Path(path) = ty else {
        return None;
    };
    path_unordered_name(&path.path)
}

/// The unordered type a path's final segment names, if it names one.
///
/// Only the final segment is read, which is what makes `HashMap` and
/// `HashMapLike` different names rather than different lengths of one name,
/// and leaves a module named `HashMap` in a non-final position unremarked: a
/// module is not a collection, and its iteration order is the type's.
fn path_unordered_name(path: &Path) -> Option<&'static str> {
    let segment = path.segments.last()?;
    unordered_ident(&segment.ident)
}

/// The unordered type any segment of a path names, if one does.
///
/// Used for value paths, where the collection's name is not final: a
/// constructor call reads `HashMap::new`. Identifiers are compared against the
/// parse tree, so a method or field that merely starts with a matched spelling
/// is a different name.
fn any_segment_unordered(path: &Path) -> Option<&'static str> {
    path.segments
        .iter()
        .find_map(|segment| unordered_ident(&segment.ident))
}

/// The unordered type `ident` names, if it names one.
///
/// A suffix match rather than equality, because the workspace aliases these
/// collections: `BuildGraph::actions` is an `IrHashMap`. No `src/ninja_gen`
/// file names the alias today, which is exactly what makes equality look
/// sufficient — the first one to do so would reach emitted bytes behind a green
/// contract. Returning the matched spelling rather than the identifier keeps
/// [`Hit::type_name`] one of [`UNORDERED_TYPES`], so the exemption list and the
/// pinning test still name the collection rather than the alias.
///
/// [`HashMapLike`] does not end in a matched spelling and stays unremarked,
/// which is the distinction [`path_unordered_name`] also draws.
///
/// [`Hit::type_name`]: Hit::type_name
fn unordered_ident(ident: &syn::Ident) -> Option<&'static str> {
    let spelled = ident.to_string();
    UNORDERED_TYPES
        .iter()
        .copied()
        .find(|type_name| spelled.ends_with(type_name))
}

/// Return whether any branch of `tree` is the type called `wanted`.
///
/// A `use` is a tree of paths sharing prefixes, and the module is named by
/// whichever segment of a branch holds it:
///
/// ```text
/// use std::collections::HashMap;
/// use std::collections::{BTreeMap, HashMap};
/// use std::collections::HashMap as Map;
/// ```
///
/// Comparing identifiers against the parse tree makes `HashMapLike` a different
/// name rather than a longer spelling of this one, and a comment quoting the
/// same words is not a `use` at all.
///
/// Matching by [`unordered_ident`] rather than by equality is what finds
/// `use crate::ir::graph::IrHashMap`, whose imported name is the alias rather
/// than the collection.
///
/// The rename arm is reached only by `use … as Y`, and **both** names it holds
/// can matter, so both are read. In `UseRename`, `ident` is the original and
/// `rename` is the alias: `use …::IrHashMap as Actions` names the collection on
/// the left, and `use …::Actions as HashMap` puts the collection's own name into
/// the file on the right. Comparing either one alone leaves a shape uncaught,
/// which is why each direction carries its own case in `scan_coverage`.
fn names_segment(tree: &UseTree, wanted: &str) -> bool {
    let names =
        |ident: &syn::Ident| unordered_ident(ident).is_some_and(|matched| matched == wanted);
    match tree {
        UseTree::Path(path) => names(&path.ident) || names_segment(&path.tree, wanted),
        UseTree::Name(name) => names(&name.ident),
        UseTree::Rename(rename) => names(&rename.ident) || names(&rename.rename),
        UseTree::Glob(_) => false,
        UseTree::Group(group) => group
            .items
            .iter()
            .any(|branch| names_segment(branch, wanted)),
    }
}
