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

use syn::{
    Attribute, Field, File, Item, Meta, Path, Type, UseTree,
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

/// Return whether `file` declares a child module called `name` only for tests.
///
/// The boundary walk calls this with a directory's own `mod.rs` and each of its
/// children: the child's *contents* carry no attribute, so the parent is the
/// only place the answer exists. A declaration the file does not carry, or one
/// with no `cfg`, answers `false` — an undeclared child cannot be found here,
/// and the walk has already restricted itself to files this repository's layout
/// contract makes declarable.
pub(super) fn declares_test_only_module(file: &File, name: &str) -> bool {
    file.items.iter().any(|item| match item {
        Item::Mod(module) => module.ident == name && is_test_only(item),
        _ => false,
    })
}

/// Return whether `item` is compiled only for tests.
fn is_test_only(item: &Item) -> bool {
    let attributes = match item {
        Item::Mod(named) => &named.attrs,
        Item::Use(named) => &named.attrs,
        Item::Struct(named) => &named.attrs,
        Item::Enum(named) => &named.attrs,
        Item::Fn(named) => &named.attrs,
        Item::Const(named) => &named.attrs,
        Item::Static(named) => &named.attrs,
        Item::Impl(named) => &named.attrs,
        Item::Type(named) => &named.attrs,
        Item::Trait(named) => &named.attrs,
        _ => return false,
    };
    attributes.iter().any(implies_test)
}

/// Return whether one `cfg` attribute holds only in the test configuration.
///
/// The predicate is read as a tree rather than as text, and the question asked
/// is whether it *implies* `test`:
///
/// - `test` does.
/// - `all(a, b)` does when either conjunct does, since both must hold.
/// - `any(a, b)` does only when both disjuncts do, since either may hold.
/// - `not(..)` never does, and neither does any other predicate.
///
/// The distinction matters at the call site: `cfg(all(test, unix))` is never
/// compiled in a release build, while `cfg(any(test, unix))` is.
fn implies_test(attribute: &Attribute) -> bool {
    if !names_cfg(attribute.path()) {
        return false;
    }
    let Ok(meta) = attribute.parse_args::<Meta>() else {
        return false;
    };
    meta_implies_test(&meta)
}

/// Return whether an attribute path is the `cfg` macro.
fn names_cfg(path: &Path) -> bool {
    path.segments
        .last()
        .is_some_and(|segment| segment.ident == "cfg")
}

/// Return whether one parsed `cfg` predicate implies the test configuration.
fn meta_implies_test(meta: &Meta) -> bool {
    match meta {
        Meta::Path(path) => is_test_ident(path),
        Meta::List(list) => list
            .parse_args_with(syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated)
            .is_ok_and(|operands| predicate_implies_test(&list.path, &operands)),
        Meta::NameValue(_) => false,
    }
}

/// Return whether a combinator's operands imply the test configuration.
///
/// `all` needs one conjunct to imply `test`; `any` needs every disjunct to.
/// Anything else — `not(...)`, `target_os = "..."` — implies nothing.
fn predicate_implies_test(
    combinator: &Path,
    operands: &syn::punctuated::Punctuated<Meta, syn::Token![,]>,
) -> bool {
    let Some(name) = combinator.segments.last() else {
        return false;
    };
    let verdicts: Vec<bool> = operands.iter().map(meta_implies_test).collect();
    match name.ident.to_string().as_str() {
        "all" => verdicts.into_iter().any(|verdict| verdict),
        "any" => verdicts.into_iter().all(|verdict| verdict),
        _ => false,
    }
}

/// Return whether a bare predicate path is the `test` configuration.
fn is_test_ident(path: &Path) -> bool {
    path.segments.len() == 1
        && path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "test")
}
