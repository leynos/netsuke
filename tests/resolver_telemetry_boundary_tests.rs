//! Contract test: the resolver domain does not depend on its telemetry layer.
//!
//! [`crate::stdlib::which::resolve_error`] describes what can go wrong while
//! resolving a command. Its `category()` used to return a `&'static str` read
//! straight out of the telemetry module's label constants, which made the
//! domain's notion of a failure and the label a metric carries the same
//! declaration. Nothing failed when that happened — every test still passed,
//! because the taxonomy *was* the label set — and that is exactly the problem:
//! renaming a metric label would silently rename a domain concept, and a
//! reader of the error type could not tell which of the two they were looking
//! at. The two are now separate — the domain owns `ResolveErrorCategory` and
//! the telemetry boundary maps it — and this crate holds that separation in
//! place.
//!
//! The assertion is about what a module says, so there is no run in which it is
//! observable: "this module does not reach into that one" is a statement about
//! the module's imports, and imports are read from the module's syntax. Each
//! source is parsed and its `use` items are read as the trees they are, so a
//! comment that discusses the rule, or a string literal that quotes an import,
//! is not mistaken for one. Text scanning reached the same answer, but only by
//! masking comments and literals first, and every form of literal it had to
//! know about was a form it could get wrong.
//!
//! The other direction — that the two taxonomies still agree on every spelling
//! — is an executable property and is asserted in `telemetry_tests` instead.
//!
//! The scan is a whole-set equality rather than a deny-list: the modules that
//! may name telemetry are enumerated, and the set found on disk must equal it.
//! A deny-list would pass when a *new* domain module reached into telemetry,
//! which is the failure this exists to catch.

use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{
    ambient_authority,
    fs_utf8::{Dir, DirEntry},
};
use syn::{File, Item, UseTree};

/// The resolver domain, relative to the workspace root.
const RESOLVER_DOMAIN: &str = "src/stdlib/which";

/// The fewest sources the walk must find for its result to mean anything.
///
/// A walk that descended nowhere, or stopped after one directory, would find
/// no importer at all and report a clean tree. The resolver domain is larger
/// than this by some margin, so ordinary growth and pruning never trip it.
const MINIMUM_DOMAIN_SOURCES: usize = 5;

/// The boundary module itself, which by definition does not import itself.
///
/// It is excluded from the importer set rather than listed in it: the scan asks
/// which modules *reach into* telemetry, and this one is the thing reached
/// into. Its own imports point the other way — it names the domain taxonomy, so
/// that the mapping from category to label lives on this side of the boundary.
const TELEMETRY_BOUNDARY_SOURCE: &str = "src/stdlib/which/telemetry.rs";

/// The module name a `use` reaches telemetry under.
///
/// This is the file stem of [`TELEMETRY_BOUNDARY_SOURCE`], which the test
/// asserts rather than leaving the two to drift: the name searched for in a
/// `use` tree and the source the walk must find are statements about one
/// module, and a rename that moved only one of them would silently stop
/// matching anything.
const TELEMETRY_MODULE: &str = "telemetry";

/// Sources that may name the telemetry module, and why.
///
/// Enumerated rather than derived: the point of the list is that each entry is
/// a decision someone made, and a new name has to be added deliberately. The
/// three roles are distinct — the boundary's caller, the module that declares
/// it, and the tests that hold it to its contract — and no fourth role exists
/// in this domain.
const PERMITTED_TELEMETRY_IMPORTERS: [&str; 4] = [
    // The resolver's caller of the boundary. Records spans and counts; it
    // names the label constants it records, which is the boundary's public
    // surface rather than the domain's internals.
    "src/stdlib/which/cache.rs",
    // Declares the module and re-exports the vocabularies the application
    // recorder admits on.
    "src/stdlib/which/mod.rs",
    // The tests that assert the boundary's contract, including the one that
    // pins the mapping between the two taxonomies.
    "src/stdlib/which/telemetry_tests.rs",
    // The counter-series cases those tests are split into.
    "src/stdlib/which/telemetry_tests/outcome_series.rs",
];

/// The submodules of `telemetry_tests` are permitted wholesale.
///
/// They are test cases for the telemetry boundary, so naming it is their
/// subject rather than a leak; listing each one by hand would mean editing
/// this file every time the tests are split further, for a decision that was
/// already made when the directory was allowed.
const PERMITTED_TELEMETRY_TEST_DIRECTORY: &str = "src/stdlib/which/telemetry_tests/";

/// Return whether `path` may name the telemetry module.
fn is_permitted(path: &str) -> bool {
    PERMITTED_TELEMETRY_IMPORTERS.contains(&path)
        || path.starts_with(PERMITTED_TELEMETRY_TEST_DIRECTORY)
}

/// Join `name` to `prefix` with a forward slash.
///
/// Every path this module compares against is written with `/`: the domain
/// constant, the boundary source, and each entry in the permitted list.
/// [`Utf8Path::join`] would instead insert the platform's separator, so on
/// Windows the walk would report `src/stdlib/which\cache.rs` and match none of
/// them — the importer set would report every source as unexpected and every
/// permission as stale, on the platform where the comparison is least able to
/// show it. Naming the separator keeps one spelling of a repository path
/// wherever the walk runs.
fn contract_path(prefix: &Utf8Path, name: &str) -> String {
    format!("{prefix}/{name}")
}

/// Collect every `.rs` source beneath `directory`, as workspace-relative paths.
///
/// Each directory is entered from its own handle — the ambient root is opened
/// once by the caller and never traversed — so the walk carries no capability
/// it does not use, and `prefix` names the position in the workspace the
/// resulting paths are reported under.
fn collect_sources(directory: &Dir, prefix: &Utf8Path) -> Result<Vec<(String, String)>> {
    let mut sources = Vec::new();
    for entry in directory
        .read_dir(".")
        .with_context(|| format!("read {prefix}"))?
    {
        sources.extend(collect_entry_sources(directory, prefix, &entry?)?);
    }
    Ok(sources)
}

/// Collect the sources one directory entry contributes, under `prefix`.
///
/// A directory contributes whatever the walk finds beneath it, a `.rs` file
/// contributes itself, and anything else contributes nothing. Splitting the
/// per-entry decision from the walk keeps each of the two readable: this one
/// answers what a single entry is, and the caller only has to know that a
/// walk's result is the concatenation of its entries'.
fn collect_entry_sources(
    directory: &Dir,
    prefix: &Utf8Path,
    entry: &DirEntry,
) -> Result<Vec<(String, String)>> {
    let name = entry.file_name().context("read entry name")?;
    let path = contract_path(prefix, &name);
    let file_type = entry.file_type().context("read entry type")?;
    if file_type.is_dir() {
        let nested = directory
            .open_dir(&name)
            .with_context(|| format!("open {path}"))?;
        return collect_sources(&nested, Utf8Path::new(&path));
    }
    if !file_type.is_file() || Utf8Path::new(&name).extension() != Some("rs") {
        return Ok(Vec::new());
    }
    let text = directory
        .read_to_string(&name)
        .with_context(|| format!("read {path}"))?;
    Ok(vec![(path, text)])
}

/// Return whether any branch of `tree` is the module called `wanted`.
///
/// A `use` item is a tree of paths sharing prefixes, and the module is named by
/// whichever segment of a branch holds it. Every position matters, because every
/// position can reach it, and the spellings the domain would actually use are
/// spread across those positions:
///
/// ```text
/// use crate::stdlib::which::telemetry;           // the last segment
/// use crate::stdlib::which::{cache, telemetry};  // a branch of a group
/// use telemetry::counters;                       // the first segment
/// use crate::stdlib::which::telemetry as alias;  // renamed, still reached
/// ```
///
/// Comparing identifiers against the parse tree is what makes the rule exact:
/// `telemetry_tests` is a different module rather than a longer spelling of
/// this one, and a comment or a string holding the same text is not a `use` at
/// all — neither distinction has to be re-stated as a matching rule here.
fn names_segment(tree: &UseTree, wanted: &str) -> bool {
    match tree {
        UseTree::Path(path) => path.ident == wanted || names_segment(&path.tree, wanted),
        UseTree::Name(name) => name.ident == wanted,
        UseTree::Rename(rename) => rename.ident == wanted,
        UseTree::Glob(_) => false,
        UseTree::Group(group) => group
            .items
            .iter()
            .any(|branch| names_segment(branch, wanted)),
    }
}

/// Return whether `file` imports the telemetry module.
///
/// Only `use` items are read, not qualified paths written at a call site. Those
/// are the same dependency, and the rule is stated over imports, so a module
/// that names `telemetry` without importing it would slip past; the resolver
/// therefore imports rather than qualifies, which is the convention this
/// encodes. It is a convention rather than an invariant because whether a
/// path's first segment is a crate or a local module is not a fact a single
/// file carries.
fn imports_telemetry(file: &File) -> bool {
    file.items.iter().any(|item| {
        matches!(item, Item::Use(item_use) if names_segment(&item_use.tree, TELEMETRY_MODULE))
    })
}

/// Every module under the resolver domain that names telemetry must be allowed to.
///
/// Reported as a set equality in both directions. An unlisted importer is the
/// domain reaching into its own reporting layer; a listed path that no longer
/// imports anything is a stale permission, which is how a rule like this rots
/// into a description of a tree that no longer exists. Neither is visible from
/// the other's assertion, so both are made here.
#[test]
fn only_the_telemetry_boundary_names_telemetry_in_the_resolver_domain() -> Result<()> {
    let workspace = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = Dir::open_ambient_dir(&workspace, ambient_authority())
        .context("open the workspace root")?;
    let domain = Utf8Path::new(RESOLVER_DOMAIN);
    let domain_dir = root
        .open_dir(domain)
        .with_context(|| format!("open {RESOLVER_DOMAIN}"))?;

    ensure!(
        Utf8Path::new(TELEMETRY_BOUNDARY_SOURCE).file_stem() == Some(TELEMETRY_MODULE),
        "{TELEMETRY_MODULE} is the module a `use` reaches {TELEMETRY_BOUNDARY_SOURCE} \
         under, so the two must name the same module"
    );

    let sources = collect_sources(&domain_dir, domain)?;
    ensure!(
        sources.len() >= MINIMUM_DOMAIN_SOURCES,
        "the walk found {} sources under {RESOLVER_DOMAIN}, fewer than the \
         {MINIMUM_DOMAIN_SOURCES} a working walk would see; the result would \
         be vacuously clean",
        sources.len()
    );

    let mut importers = BTreeSet::new();
    for (path, text) in &sources {
        let parsed = syn::parse_file(text).with_context(|| format!("parse {path}"))?;
        if path != TELEMETRY_BOUNDARY_SOURCE && imports_telemetry(&parsed) {
            importers.insert(path.clone());
        }
    }

    let unexpected: Vec<&String> = importers
        .iter()
        .filter(|path| !is_permitted(path))
        .collect();
    ensure!(
        unexpected.is_empty(),
        "every module under {RESOLVER_DOMAIN} that names telemetry must be a \
         boundary between the domain and its reporting layer; these are not: \
         {unexpected:?}. The resolver's error taxonomy is its own \
         (`ResolveErrorCategory` in `resolve_error.rs`); map it to a label at \
         the telemetry boundary rather than importing the label into the \
         domain."
    );

    // The stale-permission direction, checked over the declared list rather
    // than over the sources so a permission naming a file that no longer
    // exists is caught too.
    let stale: Vec<&str> = PERMITTED_TELEMETRY_IMPORTERS
        .iter()
        .copied()
        .filter(|path| !importers.contains(*path))
        .collect();
    ensure!(
        stale.is_empty(),
        "these sources are permitted to name telemetry but no longer do: \
         {stale:?}. Drop the permission rather than leaving a rule that \
         describes a tree this is not."
    );
    Ok(())
}

/// The module is named by the last segment of a plain path.
///
/// This is the shape the domain would reach the reporting layer with, and the
/// one the permitted list is written against.
#[test]
fn use_tree_names_the_last_segment_of_a_plain_path() -> Result<()> {
    let tree: UseTree = syn::parse_quote!(crate::stdlib::which::telemetry);
    ensure!(
        names_segment(&tree, TELEMETRY_MODULE),
        "a plain path ending in {TELEMETRY_MODULE} reaches it"
    );
    Ok(())
}

/// The module is named from inside a braced group.
///
/// The module tree spells most of its imports this way, because the resolver
/// domain is re-exported as a group rather than named one path per item.
#[test]
fn use_tree_names_a_branch_of_a_braced_group() -> Result<()> {
    let tree: UseTree = syn::parse_quote!(crate::stdlib::which::{cache, telemetry});
    ensure!(
        names_segment(&tree, TELEMETRY_MODULE),
        "a group naming {TELEMETRY_MODULE} as one of its branches reaches it"
    );
    Ok(())
}

/// The module is named from inside a group nested in a group.
///
/// A group's branches are themselves trees, so a branch that opens another
/// group has to be descended into; a scan that read only the outer group's
/// own identifiers would stop one level short.
#[test]
fn use_tree_names_a_branch_of_a_nested_group() -> Result<()> {
    let tree: UseTree = syn::parse_quote!(crate::stdlib::which::{cache::{self, key}, telemetry});
    ensure!(
        names_segment(&tree, TELEMETRY_MODULE),
        "nesting a group does not hide the branch beside it"
    );
    Ok(())
}

/// The module is named by the segment that opens the path.
///
/// A `use telemetry::counters;` reaches the module from the root position
/// rather than the leaf, so a rule that read only each branch's final
/// identifier would miss every import that takes something out of the module
/// rather than the module itself.
#[test]
fn use_tree_names_the_opening_segment_of_a_path() -> Result<()> {
    let tree: UseTree = syn::parse_quote!(telemetry::counters);
    ensure!(
        names_segment(&tree, TELEMETRY_MODULE),
        "a path opening on {TELEMETRY_MODULE} reaches it"
    );
    Ok(())
}

/// The module is named by a renamed import, which a rename does not change.
///
/// `use ...::telemetry as reporting;` binds a local name to the module, so the
/// path still reaches it. The rename is what the module is called here, not
/// what it is.
#[test]
fn use_tree_names_a_renamed_import() -> Result<()> {
    let tree: UseTree = syn::parse_quote!(crate::stdlib::which::telemetry as reporting);
    ensure!(
        names_segment(&tree, TELEMETRY_MODULE),
        "renaming the binding does not stop the path reaching {TELEMETRY_MODULE}"
    );
    Ok(())
}

/// A module whose name merely begins with the same text is not the module.
///
/// `telemetry_tests` is a sibling of the boundary rather than a spelling of
/// it, and the resolver's own test modules are exactly the importers this rule
/// must distinguish. Comparing identifiers is what makes the distinction,
/// where text matching had to state it as a whole-word condition.
#[test]
fn use_tree_does_not_name_a_module_that_only_begins_with_the_text() -> Result<()> {
    let tree: UseTree = syn::parse_quote!(crate::stdlib::which::telemetry_tests);
    ensure!(
        !names_segment(&tree, TELEMETRY_MODULE),
        "{TELEMETRY_MODULE}_tests is a different module from {TELEMETRY_MODULE}"
    );
    Ok(())
}

/// A group naming only siblings does not name the module.
///
/// The negative direction of the group case, so that a rule returning true for
/// every `use` in the file could not satisfy the pair.
#[test]
fn use_tree_does_not_name_the_module_when_no_branch_does() -> Result<()> {
    let tree: UseTree = syn::parse_quote!(crate::stdlib::which::{cache, resolve_error});
    ensure!(
        !names_segment(&tree, TELEMETRY_MODULE),
        "no branch of this group reaches {TELEMETRY_MODULE}"
    );
    Ok(())
}
