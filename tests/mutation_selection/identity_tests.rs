//! Probes for how the reader keys an implementation and its atoms.
//!
//! Two identities matter and neither is the bare method name:
//!
//! - a method is identified by its owning type as well as its name, because two
//!   types can each declare `output_index`;
//! - a `name = "value"` cfg atom is identified by the whole pair, because
//!   `feature = "alpha"` and `feature = "beta"` are different atoms.
//!
//! Keying either by the shorter form merges distinct things, and a skip or a
//! condition on one then answers for the other. Split from `scope_tests` to keep
//! each file inside the repository's 400-line limit.

use anyhow::{Context, Result, ensure};

use super::implementations_by_name;

#[test]
fn the_reader_keys_selections_by_owner_not_by_method_name() -> Result<()> {
    // Two types may each declare `output_index`. Keying the selection by the
    // bare method name would merge them, so a skip on one type's method would
    // satisfy a check about the other's — and the protected method would go on
    // producing false survivors while the contract passed.
    let source = "\
struct BuildGraph;
impl BuildGraph {
    #[cfg(kani)]
    #[mutants::skip]
    fn output_index(&self) -> u32 { 1 }
}

struct Other;
impl Other {
    fn output_index(&self) -> u32 { 2 }
}
";
    let by_name = implementations_by_name(source)?;
    let graph = by_name
        .get("BuildGraph::output_index")
        .context("the probe must declare `BuildGraph::output_index`")?;
    ensure!(
        (graph.skipped, graph.selected) == (1, 0),
        "the graph's own `output_index` carries the skip; got {graph:?}"
    );

    let other = by_name
        .get("Other::output_index")
        .context("the probe must declare `Other::output_index`")?;
    ensure!(
        (other.skipped, other.selected) == (0, 1),
        "the other type's `output_index` is untouched by that skip, so it must \\
         stay selected under its own key; got {other:?}"
    );
    Ok(())
}

#[test]
fn the_reader_decides_feature_predicates() -> Result<()> {
    // `name = "value"` is a predicate in its own right, and the model keys it by
    // the whole `name = value` pair so two feature names are distinct atoms.
    // Keying it by the bare name would find no matching atom at evaluation and
    // report every feature gate as undecidable.
    let source = "\
impl Demo {
    #[cfg(feature = \"alpha\")]
    fn alpha(&self) -> u32 { 1 }

    #[cfg(all(feature = \"alpha\", feature = \"beta\"))]
    fn both(&self) -> u32 { 2 }

    #[cfg(kani)]
    fn gated(&self) -> u32 { 3 }
}
";
    let by_name = implementations_by_name(source)?;
    for name in ["Demo::alpha", "Demo::both"] {
        let selection = by_name
            .get(name)
            .with_context(|| format!("the probe must declare `{name}`"))?;
        ensure!(
            (selection.undecidable, selection.selected_production) == (0, 1),
            "`{name}` is a feature gate that can hold without `kani`, so it \\
             must be decided as production rather than undecidable; got \\
             {selection:?}"
        );
    }

    let gated = by_name
        .get("Demo::gated")
        .context("the probe must declare `Demo::gated`")?;
    ensure!(
        (gated.undecidable, gated.selected_production) == (0, 0),
        "`Demo::gated` requires `kani`; got {gated:?}"
    );
    Ok(())
}
