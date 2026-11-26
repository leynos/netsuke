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

#[test]
fn the_reader_normalizes_raw_attribute_names() -> Result<()> {
    // `#[r#cfg(kani)]` is `#[cfg(kani)]` to rustc: the raw marker is spelling,
    // not identity. Reading the attribute name without normalizing would let it
    // bypass the model, so a Kani-gated item would be counted as production.
    let source = "\
impl Demo {
    #[r#cfg(kani)]
    fn raw_attr(&self) -> u32 { 1 }

    #[cfg(r#kani)]
    fn raw_atom(&self) -> u32 { 2 }

    #[r#cfg_attr(not(kani), cfg(any()))]
    fn raw_cfg_attr(&self) -> u32 { 3 }
}
";
    let by_name = implementations_by_name(source)?;
    for name in ["Demo::raw_attr", "Demo::raw_atom"] {
        let selection = by_name
            .get(name)
            .with_context(|| format!("the probe must declare `{name}`"))?;
        ensure!(
            (selection.selected, selection.selected_production) == (1, 0),
            "`{name}` is Kani-gated under a raw spelling, so it must not count \
             as production; got {selection:?}"
        );
    }

    let raw_cfg_attr = by_name
        .get("Demo::raw_cfg_attr")
        .context("the probe must declare `Demo::raw_cfg_attr`")?;
    ensure!(
        raw_cfg_attr.undecidable == 1,
        "a raw-spelled `cfg_attr` still expands to whatever it names, so it \
         must reach the conservative fallback; got {raw_cfg_attr:?}"
    );
    Ok(())
}

#[test]
fn the_reader_keeps_a_qualified_self_type_distinct() -> Result<()> {
    // `other::BuildGraph` is a different type from `BuildGraph`, and the pinned
    // tool keeps the qualified name in its implementation namespace. The two
    // `impl` blocks below sit at the same lexical level, so only the `impl`
    // path distinguishes them — keying by the last segment alone would merge
    // them and let the skinned method's skip count against the other type.
    let source = "\
struct BuildGraph;
impl BuildGraph {
    #[cfg(kani)]
    #[mutants::skip]
    fn insert_edge(&self) -> u32 { 1 }
}

mod other {
    pub struct BuildGraph;
}

impl other::BuildGraph {
    pub fn insert_edge(&self) -> u32 { 2 }
}
";
    let by_name = implementations_by_name(source)?;
    let local = by_name
        .get("BuildGraph::insert_edge")
        .context("the probe must declare `BuildGraph::insert_edge`")?;
    ensure!(
        (local.skipped, local.selected) == (1, 0),
        "the local type's method carries the skip; got {local:?}"
    );

    let qualified = by_name
        .get("other::BuildGraph::insert_edge")
        .context("the probe must declare `other::BuildGraph::insert_edge`")?;
    ensure!(
        (qualified.skipped, qualified.selected_production) == (0, 1),
        "the path-qualified type is a distinct implementation and must not \
         inherit that skip; got {qualified:?}"
    );
    Ok(())
}

#[test]
fn the_reader_keeps_generic_arguments_in_the_owner_key() -> Result<()> {
    // `Demo<u8>` and `Demo<u16>` are different types. Keying by the nominal
    // name alone would merge them, so a skip on one specialization's method
    // would appear to answer for the other's.
    let source = "\
struct Demo<T>(T);

impl Demo<u8> {
    #[cfg(kani)]
    #[mutants::skip]
    fn value(&self) -> u32 { 1 }
}

impl Demo<u16> {
    fn value(&self) -> u32 { 2 }
}
";
    let by_name = implementations_by_name(source)?;
    let narrow = by_name
        .get("Demo<u8>::value")
        .context("the probe must declare `Demo<u8>::value`")?;
    ensure!(
        (narrow.skipped, narrow.selected) == (1, 0),
        "the `u8` specialization carries the skip; got {narrow:?}"
    );

    let wide = by_name
        .get("Demo<u16>::value")
        .context("the probe must declare `Demo<u16>::value`")?;
    ensure!(
        (wide.skipped, wide.selected_production) == (0, 1),
        "the `u16` specialization is a distinct implementation and must stay \\
         selected; got {wide:?}"
    );
    Ok(())
}

#[test]
fn the_reader_separates_trait_implementations_from_inherent_ones() -> Result<()> {
    // cargo-mutants gives a trait implementation its own namespace. An
    // inherent `Demo::paired` and `<impl OtherTrait for Demo>::paired` are
    // different implementations, so one must not answer for the other.
    let source = "\
struct Demo;

impl Demo {
    #[cfg(kani)]
    #[mutants::skip]
    fn paired(&self) -> u32 { 1 }
}

trait OtherTrait { fn paired(&self) -> u32; }

impl OtherTrait for Demo {
    fn paired(&self) -> u32 { 2 }
}
";
    let by_name = implementations_by_name(source)?;
    let inherent = by_name
        .get("Demo::paired")
        .context("the probe must declare the inherent `Demo::paired`")?;
    ensure!(
        (inherent.skipped, inherent.selected) == (1, 0),
        "the inherent method carries the skip; got {inherent:?}"
    );

    let trait_impl = by_name
        .get("<impl OtherTrait for Demo>::paired")
        .context("the probe must declare the trait implementation")?;
    ensure!(
        (trait_impl.skipped, trait_impl.selected_production) == (0, 1),
        "the trait method is a separate implementation and must stay selected; \\
         got {trait_impl:?}"
    );
    Ok(())
}

#[test]
fn the_reader_puts_the_module_outside_the_trait_wrapper() -> Result<()> {
    // `m::<impl Trait for Demo>` and `<impl Trait for m::Demo>` are different
    // implementations, and cargo-mutants keeps them apart by building the
    // implementation component first and prepending the module path. Wrapping
    // an already-qualified type would merge them.
    let source = "\
trait Trait { fn paired(&self) -> u32; }
struct Demo;

mod m {
    use super::Trait;

    pub struct Demo;
    #[cfg(kani)]
    #[mutants::skip]
    impl Trait for Demo {
        fn paired(&self) -> u32 { 1 }
    }
}

impl Trait for m::Demo {
    fn paired(&self) -> u32 { 2 }
}
";
    let by_name = implementations_by_name(source)?;
    let nested = by_name
        .get("m::<impl Trait for Demo>::paired")
        .context("the probe must declare `m::<impl Trait for Demo>::paired`")?;
    ensure!(
        (nested.skipped, nested.selected) == (1, 0),
        "the module-scoped implementation carries the skip; got {nested:?}"
    );

    let qualified = by_name
        .get("<impl Trait for m::Demo>::paired")
        .context("the probe must declare `<impl Trait for m::Demo>::paired`")?;
    ensure!(
        (qualified.skipped, qualified.selected_production) == (0, 1),
        "the root-level implementation for a module-qualified type is distinct \
         and must stay selected; got {qualified:?}"
    );
    Ok(())
}
