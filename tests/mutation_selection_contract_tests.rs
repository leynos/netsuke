//! Selection contract: only proof-only graph implementations leave the
//! ordinary mutation lane.
//!
//! `cargo-mutants` judges a mutant by running the ordinary test binaries, so an
//! implementation that exists only under `cfg(kani)` is invisible to that
//! judgement: no mutation of it can fail a test, and every such mutant is
//! reported as a survivor that says nothing about test quality. Issue #1038
//! records six of them in `src/ir/graph/mod.rs`.
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
//! implementations now carry. This test holds the properties that make the
//! choice correct rather than merely present:
//!
//! 1. Every skipped implementation in the graph module is `cfg(kani)`, so a
//!    skip cannot land on code the ordinary lane genuinely judges.
//! 2. Each of the four names still carries a skip, so the ordinary lane stops
//!    reporting survivors it cannot judge.
//! 3. The three names with a production counterpart keep it selected, so a
//!    future edit cannot widen a skip into a whole-name exclusion.
//! 4. Shared graph logic the test lane judges stays selected.
//!
//! The parser and selection logic live in the [`mutation_selection`] helper
//! module, which documents the cargo-mutants rules it models. It is a separate
//! file so parser maintenance stays apart from these requirements, and so each
//! stays inside the repository's 400-line limit.
//!
//! The caller workflow's side of the same arrangement — that its file-scoped
//! `exclude-globs` is not pointed at the mixed file — is asserted by
//! `tests/workflow_contracts/mutation_exclusion_scope_test.py`, where the
//! `with:` block is already parsed. Asserting it here as well would duplicate
//! one contract in two languages.
//!
//! The fixture in `tests/data/mutation_selection/` is the independent control.
//! It is a source file this repository does not compile, holding a same-named
//! Kani/production pair, a same-named pair where the *production* half is
//! skipped, and a pair whose enclosing `impl` block is skipped. The reader is
//! applied to it, and the fixture proves the reader distinguishes the three: a
//! reader that answered "has a selected implementation" for every name would
//! pass on the real module and be wrong, which is exactly the failure a broad
//! name-based exclusion produces.

#[path = "mutation_selection/reader.rs"]
mod mutation_selection;

use anyhow::{Context, Result, ensure};
use cap_std::{ambient_authority, fs_utf8::Dir};
use mutation_selection::{Selection, implementations_by_name};

/// The module that mixes proof-only and production graph implementations.
const GRAPH_MODULE: &str = "src/ir/graph/mod.rs";

/// A same-named Kani/production pair whose Kani half is skipped, a same-named
/// pair whose production half is skipped, and a pair under a skipped `impl`.
const FIXTURE: &str = "tests/data/mutation_selection/same_named_pairs.rs";

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
        // `selected_production` rather than `selected`: a selected `cfg(kani)`
        // implementation is not a production counterpart, so counting it would
        // pass a name whose remaining implementation moved behind the Kani gate.
        let keeps_twin = selection.is_some_and(|found| found.selected_production > 0);
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
            // `selected_production`, not `selected`: a `cfg(kani)`-only
            // implementation would otherwise satisfy a check whose whole point
            // is that the ordinary lane judges the code.
            by_name
                .get(*name)
                .is_none_or(|selection| selection.selected_production == 0)
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
fn the_fixture_covers_an_enclosing_skip() -> Result<()> {
    // The fixture's third control: an impl block whose methods carry no
    // attribute of their own. It exists so the enclosing-skip rule has a
    // committed case as well as the inline probes above.
    let by_name = implementations_by_name(&fixture_source()?)?;
    for name in ["under_skipped_impl", "also_under_skipped_impl"] {
        let selection = by_name
            .get(name)
            .with_context(|| format!("the fixture must declare `{name}`"))?;
        ensure!(
            (selection.selected, selection.skipped) == (0, 1),
            "`{name}` sits in a skipped impl block and must read as skipped, \
             not selected; got {selection:?}"
        );
    }
    Ok(())
}
