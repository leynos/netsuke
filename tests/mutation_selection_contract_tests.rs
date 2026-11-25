//! Selection contract: only proof-only graph implementations leave the
//! ordinary mutation lane.
//!
//! `cargo-mutants` judges a mutant by running the ordinary test binaries, so an
//! implementation that exists only under `cfg(kani)` is invisible to that
//! judgement: no mutation of it can fail a test, and every such mutant is
//! reported as a survivor that says nothing about test quality. Issue #1038
//! records six of them in [`src/ir/graph/mod.rs`](../../src/ir/graph/mod.rs).
//!
//! The obvious repairs are all too broad or too narrow, and the difference only
//! shows up when the declaration is read as syntax. `exclude-globs` is file
//! scoped, so it would take the production counterparts with it. A name-based
//! `mutants::exclude_re` or `--exclude-re` cannot separate the halves either:
//! `BuildGraph::edge_id_for_output` and `BuildGraph::target_for_output` each
//! have a `cfg(kani)` implementation and a `cfg(not(kani))` one, and
//! cargo-mutants names both implementations of a pair identically. An `impl`-
//! or `mod`-scoped skip is broader still.
//!
//! What remains is a per-item attribute, and that is what the four proof-only
//! implementations now carry. This test holds the two properties that make the
//! choice correct rather than merely present:
//!
//! 1. Every skipped implementation in the graph module is `cfg(kani)`, so a
//!    skip cannot land on code the ordinary lane genuinely judges.
//! 2. Each of the four names still has a selected, non-skipped
//!    implementation, so a future edit cannot quietly widen a skip into a
//!    whole-name exclusion.
//!
//! The fixture in [`tests/data/mutation_selection/`](mutation_selection) is the
//! second property's independent control. It is a source file this repository
//! does not compile, holding a same-named Kani/production pair and a
//! same-named pair where the *production* half is skipped. The reader below is
//! applied to both, and the fixture proves it distinguishes them: a reader that
//! answered "has a selected implementation" for every name would pass on the
//! real module and be wrong, which is exactly the failure a broad name-based
//! exclusion produces.
//!
//! The declaration is read as syntax rather than as text, so a doc comment
//! explaining a skip, or a string quoting one, is not mistaken for the
//! attribute.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs_utf8::Dir};
use syn::{Attribute, File, ImplItem, Item, ItemImpl, Meta};

/// The module that mixes proof-only and production graph implementations.
const GRAPH_MODULE: &str = "src/ir/graph/mod.rs";

/// A same-named Kani/production pair whose Kani half is skipped, and a
/// same-named pair whose production half is skipped.
const FIXTURE: &str = "tests/data/mutation_selection/same_named_pairs.rs";

/// One implementation found in an `impl` block.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Implementation {
    /// The method name cargo-mutants would print for this implementation.
    name: String,
    /// True when the item carries an item-level `mutants::skip`.
    skipped: bool,
    /// True when the item is gated to the Kani runner.
    kani_only: bool,
}

/// What one method name resolves to across all its implementations.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct Selection {
    /// Implementations of this name that stay in the ordinary lane.
    selected: usize,
    /// Implementations of this name that carry a skip.
    skipped: usize,
    /// Skipped implementations that are not `cfg(kani)`.
    skipped_outside_kani: usize,
}

/// True when `attr` is, or contains as a bare path condition, `mutants::skip`.
///
/// Mirrors the rule cargo-mutants applies. Its reader accepts an attribute path
/// of exactly `mutants::skip`, or a `cfg_attr` whose first argument is a bare
/// path and whose remaining arguments include `mutants::skip`. A `cfg_attr`
/// condition written with parentheses — `any()`, `all(kani, test)` — is not a
/// path, so the whole attribute is rejected and a skip written inside it is
/// silently dropped. Recognising only the accepted forms here is what stops
/// this contract from certifying a skip cargo-mutants would ignore.
fn is_mutants_skip(attr: &Attribute) -> bool {
    match &attr.meta {
        Meta::Path(path) => path_is_mutants_skip(path),
        Meta::List(list) => {
            if !path_is_cfg_attr(&list.path) {
                return false;
            }
            let mut skip = false;
            let parsed = attr.parse_nested_meta(|meta| {
                if path_is_mutants_skip(&meta.path) {
                    skip = true;
                }
                Ok(())
            });
            parsed.is_ok() && skip
        }
        Meta::NameValue(_) => false,
    }
}

/// True when `path` is exactly `mutants::skip`.
fn path_is_mutants_skip(path: &syn::Path) -> bool {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .eq(["mutants".to_owned(), "skip".to_owned()])
}

/// True when `path` is exactly `cfg_attr`.
fn path_is_cfg_attr(path: &syn::Path) -> bool {
    path.is_ident("cfg_attr")
}

/// True when `attrs` gate the item to the Kani runner.
///
/// A `cfg_attr` carrying a `kani` condition is deliberately not treated as a
/// Kani gate: the ordinary lane does not evaluate it, so such an item would be
/// present in the test binaries and must keep its mutants.
fn is_kani_only(attrs: &[Attribute]) -> bool {
    attrs.iter().any(is_direct_kani_cfg)
}

/// True when `attr` is a `cfg` attribute naming `kani` as one of its conditions.
fn is_direct_kani_cfg(attr: &Attribute) -> bool {
    let Meta::List(list) = &attr.meta else {
        return false;
    };
    if !list.path.is_ident("cfg") {
        return false;
    }
    let mut found = false;
    let parsed = attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("kani") {
            found = true;
        }
        Ok(())
    });
    parsed.is_ok() && found
}

/// Collect every implementation in `source`, grouped by method name.
fn implementations_by_name(source: &str) -> Result<BTreeMap<String, Selection>> {
    let parsed: File = syn::parse_str(source).context("parse Rust source as a file")?;
    let mut by_name: BTreeMap<String, Selection> = BTreeMap::new();
    for item in &parsed.items {
        let Item::Impl(block) = item else {
            continue;
        };
        accumulate_impl(block, &mut by_name);
    }
    Ok(by_name)
}

/// Fold one `impl` block's methods into `by_name`.
fn accumulate_impl(block: &ItemImpl, by_name: &mut BTreeMap<String, Selection>) {
    for item in &block.items {
        let ImplItem::Fn(method) = item else {
            continue;
        };
        let implementation = Implementation {
            name: method.sig.ident.to_string(),
            skipped: method.attrs.iter().any(is_mutants_skip),
            kani_only: is_kani_only(&method.attrs),
        };
        let entry = by_name.entry(implementation.name).or_default();
        if implementation.skipped {
            entry.skipped += 1;
            if !implementation.kani_only {
                entry.skipped_outside_kani += 1;
            }
        } else {
            entry.selected += 1;
        }
    }
}

/// Return a repository-relative source file's text.
///
/// Reads through a capability opened on the crate root rather than through
/// `std::fs`, so the test names the directory it is allowed to read from and
/// nothing outside it. The repository forbids the ambient `std::fs` entry
/// points for the same reason production code does: a path built by
/// concatenation can reach anywhere, while a capability cannot.
fn repository_source(relative: &str) -> Result<String> {
    let root = Dir::open_ambient_dir(env!("CARGO_MANIFEST_DIR"), ambient_authority())
        .context("open the crate root")?;
    root.read_to_string(relative)
        .with_context(|| format!("read {relative}"))
}

/// Return the graph module's source, read from the crate root.
fn graph_source() -> Result<String> {
    repository_source(GRAPH_MODULE)
}

/// Return the fixture's source, read from the crate root.
fn fixture_source() -> Result<String> {
    repository_source(FIXTURE)
}

/// The four proof-only implementations issue #1038 reported as false survivors.
///
/// Three of them are one half of a same-named pair, so the property that
/// matters for those is that the *other* half stays selected.
const PROOF_ONLY_NAMES: [&str; 4] = [
    "insert_edge",
    "output_index",
    "edge_id_for_output",
    "target_for_output",
];

/// The subset of [`PROOF_ONLY_NAMES`] whose production counterpart exists.
///
/// `output_index` is deliberately absent: it is a Kani-only accessor with no
/// `cfg(not(kani))` twin, because the production duplicate-output scan reads
/// the target map directly. A blanket "every proof-only name keeps a selected
/// twin" rule would demand a production method that does not exist, and adding
/// one to satisfy a test would be the test dictating the design.
const PROOF_ONLY_WITH_PRODUCTION_TWIN: [&str; 3] =
    ["insert_edge", "edge_id_for_output", "target_for_output"];

/// Named shared or production implementations that must stay selected.
const STAYS_SELECTED: [&str; 7] = [
    "insert_canonical_edge",
    "index_output_aliases",
    "index_output",
    "replace_edge_for_output",
    "duplicate_output",
    "edge_count",
    "output_count",
];

#[test]
fn every_skipped_graph_implementation_is_kani_only() -> Result<()> {
    let by_name = implementations_by_name(&graph_source()?)?;
    let offenders: Vec<(&str, usize)> = by_name
        .iter()
        .filter(|(_, selection)| selection.skipped_outside_kani > 0)
        .map(|(name, selection)| (name.as_str(), selection.skipped_outside_kani))
        .collect();

    ensure!(
        offenders.is_empty(),
        "a skip may only mark a cfg(kani) implementation, because the ordinary \
         test lane is what it removes the item from; these skips sit on code \
         that lane compiles: {offenders:?}"
    );
    Ok(())
}

#[test]
fn each_proof_only_name_is_skipped_exactly_once() -> Result<()> {
    let by_name = implementations_by_name(&graph_source()?)?;
    let mut unskipped: Vec<&str> = Vec::new();
    for name in PROOF_ONLY_NAMES {
        if by_name.get(name).is_none_or(|found| found.skipped == 0) {
            unskipped.push(name);
        }
    }

    ensure!(
        unskipped.is_empty(),
        "these proof-only implementations must carry their own skip, or the \
         ordinary lane reports them as survivors it cannot judge: {unskipped:?}"
    );
    Ok(())
}

#[test]
fn each_paired_proof_only_name_keeps_a_selected_production_twin() -> Result<()> {
    let by_name = implementations_by_name(&graph_source()?)?;
    let mut unselected: Vec<(&str, Option<&Selection>)> = Vec::new();
    for name in PROOF_ONLY_WITH_PRODUCTION_TWIN {
        let selection = by_name.get(name);
        let keeps_twin = selection.is_some_and(|found| found.selected > 0);
        if !keeps_twin {
            unselected.push((name, selection));
        }
    }

    ensure!(
        unselected.is_empty(),
        "each of these names has a cfg(not(kani)) counterpart that the ordinary \
         lane genuinely judges, so excluding the name outright would drop real \
         mutants; found {unselected:?}"
    );
    Ok(())
}

#[test]
fn shared_graph_logic_stays_in_the_ordinary_lane() -> Result<()> {
    let by_name = implementations_by_name(&graph_source()?)?;
    let skipped: Vec<&str> = STAYS_SELECTED
        .iter()
        .copied()
        .filter(|name| {
            by_name
                .get(*name)
                .is_none_or(|selection| selection.selected == 0)
        })
        .collect();

    ensure!(
        skipped.is_empty(),
        "these implementations are ordinary graph logic the test lane judges, \
         so they must not be skipped: {skipped:?}"
    );
    Ok(())
}

#[test]
fn the_fixture_distinguishes_which_half_of_a_pair_is_skipped() -> Result<()> {
    let by_name = implementations_by_name(&fixture_source()?)?;

    let skipped_kani = by_name
        .get("skipped_kani_half")
        .context("the fixture must declare `skipped_kani_half`")?;
    ensure!(
        (skipped_kani.selected, skipped_kani.skipped) == (1, 1),
        "the fixture's Kani-skipped pair must resolve to one selected and one \
         skipped implementation, so the reader proves it keeps the production \
         half; got {skipped_kani:?}"
    );

    let skipped_production = by_name
        .get("skipped_production_half")
        .context("the fixture must declare `skipped_production_half`")?;
    ensure!(
        (
            skipped_production.selected,
            skipped_production.skipped,
            skipped_production.skipped_outside_kani,
        ) == (1, 1, 1),
        "the fixture's production-skipped pair must report its skip as falling \
         outside Kani, which is the shape \
         `every_skipped_graph_implementation_is_kani_only` rejects; got \
         {skipped_production:?}"
    );
    Ok(())
}

#[test]
fn the_reader_ignores_a_skip_that_cargo_mutants_would_drop() -> Result<()> {
    // `cfg_attr(any(), mutants::skip)` is not honoured by cargo-mutants: its
    // reader accepts only a bare-path condition, so a skip written inside a
    // parenthesised one is silently discarded. A contract that counted it as a
    // skip would certify an exclusion the pinned tool does not perform.
    let source = "\
impl Demo {
    #[cfg_attr(any(), mutants::skip)]
    fn parenthesised_condition(&self) -> u32 { 1 }

    #[cfg_attr(test, mutants::skip)]
    fn bare_path_condition(&self) -> u32 { 2 }
}
";
    let by_name = implementations_by_name(source)?;
    let dropped = by_name
        .get("parenthesised_condition")
        .context("the probe must declare `parenthesised_condition`")?;
    ensure!(
        (dropped.selected, dropped.skipped) == (1, 0),
        "a skip under a parenthesised cfg_attr condition must read as selected, \
         because cargo-mutants discards it; got {dropped:?}"
    );

    let honoured = by_name
        .get("bare_path_condition")
        .context("the probe must declare `bare_path_condition`")?;
    ensure!(
        (honoured.selected, honoured.skipped) == (0, 1),
        "a skip under a bare-path cfg_attr condition must read as skipped, \
         because cargo-mutants honours it; got {honoured:?}"
    );
    Ok(())
}

#[test]
fn the_four_skips_cover_the_mixed_file_the_workflow_cannot_exclude() -> Result<()> {
    // The per-item skips are only the right mechanism if the caller's
    // file-scoped excludes cannot reach this file. The caller's own contract
    // test holds the `with:` block; this asserts the complementary fact the
    // Rust side depends on, reading the declared `exclude-globs` value as data
    // rather than grepping the document, because the workflow's explanatory
    // comment names the mixed file on purpose.
    let exclude_globs = declared_exclude_globs()?;
    let globs: BTreeSet<&str> = exclude_globs
        .split(',')
        .map(|glob| glob.trim().trim_matches('"'))
        .filter(|glob| !glob.is_empty())
        .collect();

    ensure!(
        !globs.contains(GRAPH_MODULE),
        "{GRAPH_MODULE} must not appear in exclude-globs: it holds both \
         proof-only and production implementations, so a path exclusion would \
         remove the counterparts too; got {globs:?}"
    );
    for whole_file in [
        "src/ir/cycle/verification.rs",
        "src/ir/cmd_interpolate/verification.rs",
        "src/ir/from_manifest/verification.rs",
        "src/ir/graph/kani_map.rs",
    ] {
        ensure!(
            globs.contains(whole_file),
            "the caller must keep excluding the whole Kani-only file \
             {whole_file}, which no per-item skip replaces; got {globs:?}"
        );
    }
    Ok(())
}

/// Return the caller workflow's declared `exclude-globs` value.
///
/// Reads the line that assigns the key rather than the whole document, so the
/// explanatory comment around it cannot be mistaken for configuration. The YAML
/// block scalar that follows is indented, so the first non-indented line ends
/// the value.
fn declared_exclude_globs() -> Result<String> {
    let text = repository_source(".github/workflows/mutation-testing.yml")?;

    let mut value = String::new();
    let mut in_value = false;
    for line in text.lines() {
        if in_value {
            let trimmed = line.trim_start();
            if line.starts_with(char::is_whitespace) && !trimmed.starts_with('#') {
                value.push_str(trimmed.trim_end_matches('\\'));
                continue;
            }
            break;
        }
        if let Some(rest) = line.trim_start().strip_prefix("exclude-globs:") {
            in_value = true;
            value.push_str(rest.trim().trim_end_matches('\\'));
        }
    }

    ensure!(
        !value.is_empty(),
        "the caller workflow must declare a non-empty exclude-globs value"
    );
    Ok(value)
}
