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

#[test]
fn the_reader_keys_types_by_their_module_path() -> Result<()> {
    // Two modules may each declare a type of the same name. cargo-mutants
    // prints the module path, so a key that dropped it would merge the two and
    // let a selected method on one satisfy a check about the other.
    let source = "\
mod left {
    struct BuildGraph;
    impl BuildGraph {
        fn insert_edge(&self) -> u32 { 1 }
    }
}

mod right {
    struct BuildGraph;
    impl BuildGraph {
        #[cfg(kani)]
        #[mutants::skip]
        fn insert_edge(&self) -> u32 { 2 }
    }
}
";
    let by_name = implementations_by_name(source)?;
    let left = by_name
        .get("left::BuildGraph::insert_edge")
        .context("the probe must declare `left::BuildGraph::insert_edge`")?;
    ensure!(
        (left.skipped, left.selected) == (0, 1),
        "the left module's method carries no skip; got {left:?}"
    );

    let right = by_name
        .get("right::BuildGraph::insert_edge")
        .context("the probe must declare `right::BuildGraph::insert_edge`")?;
    ensure!(
        (right.skipped, right.selected) == (1, 0),
        "the right module's method carries the skip and must not be merged \
         with the left one; got {right:?}"
    );
    Ok(())
}

#[test]
fn the_reader_excludes_default_implementations() -> Result<()> {
    // The walker returns from `visit_item_impl` for any `Default`
    // implementation, so nothing inside it is mutable or selectable. Counting
    // its methods as production would hand an inherent skipped `default` a
    // false counterpart.
    let source = "\
struct Demo { value: u32 }

impl Default for Demo {
    fn default() -> Self { Demo { value: 1 } }
}

impl Demo {
    fn inherent(&self) -> u32 { 2 }
}
";
    let by_name = implementations_by_name(source)?;
    ensure!(
        by_name.is_empty() || !by_name.keys().any(|key| key.ends_with("::default")),
        "a `Default` implementation produces no mutants, so its methods must \
         not appear as selected; got {:?}",
        by_name.keys().collect::<Vec<_>>()
    );

    let inherent = by_name
        .get("Demo::inherent")
        .context("the probe must declare `Demo::inherent`")?;
    ensure!(
        (inherent.skipped, inherent.selected) == (0, 1),
        "an ordinary inherent method must stay selected, so the exclusion above \
         is specific to the trait block; got {inherent:?}"
    );
    Ok(())
}
