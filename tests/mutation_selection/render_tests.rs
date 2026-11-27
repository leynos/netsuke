//! Probes for how an `impl` block's identity is rendered.
//!
//! The owner key is text rendered from the self type, the lexical module path,
//! and the implemented trait. Every distinction the rendering loses merges two
//! implementations, and a skip on one then answers a check about the other — so
//! these probes pin the distinctions that are easy to lose: generic arguments,
//! the contents of a const-generic literal, and the placement of the module
//! path relative to the trait wrapper.
//!
//! Split from `identity_tests` to keep each file inside the repository's
//! 400-line limit.

use anyhow::{Context, Result, ensure};

use super::implementations_by_name;

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

#[test]
fn the_reader_keeps_literal_contents_distinct() -> Result<()> {
    // Two self types whose const-generic literals differ only by spacing around
    // `::` are different types. Normalizing the rendered text as a whole would
    // rewrite the literal and merge them, so a skip on one would appear to
    // leave a production counterpart on the other.
    let source = "\
struct Demo<const N: usize>;

impl Demo<{ \"a :: b\".len() }> {
    #[cfg(kani)]
    #[mutants::skip]
    fn value(&self) -> u32 { 1 }
}

impl Demo<{ \"a::b\".len() }> {
    fn value(&self) -> u32 { 2 }
}
";
    let by_name = implementations_by_name(source)?;
    // The key's exact spacing is an implementation detail; what matters is that
    // the two literals yield two entries. Each literal's own text survives, so
    // the spaced one contains `a :: b` and the tight one contains `a::b`.
    let spaced = by_name
        .iter()
        .find(|(key, _)| key.contains("\"a :: b\""))
        .map(|(_, selection)| selection);
    let tight = by_name
        .iter()
        .find(|(key, _)| key.contains("\"a::b\""))
        .map(|(_, selection)| selection);

    ensure!(
        spaced.is_some() && tight.is_some(),
        "each literal's contents must survive into its own key, or the two \
         specializations merge; got {:?}",
        by_name.keys().collect::<Vec<_>>()
    );
    ensure!(
        spaced.is_some_and(|selection| (selection.skipped, selection.selected) == (1, 0)),
        "the spaced-literal specialization carries the skip; got {spaced:?}"
    );
    ensure!(
        tight.is_some_and(|selection| (selection.skipped, selection.selected_production) == (0, 1)),
        "the tight-literal specialization is a different type and must stay \
         selected; got {tight:?}"
    );
    Ok(())
}
