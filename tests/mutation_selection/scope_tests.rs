//! Probes for how the reader inherits decisions from enclosing scopes.
//!
//! These cover the rules that are not visible on a method alone: an exclusion
//! or a Kani gate declared on the file, on an inline module, or on the `impl`
//! block, and the eligibility checks that remove a method without any attribute
//! at all. They live beside `tests`, split from it to keep each file inside the
//! repository's 400-line limit; the attribute-level probes stay there.
//!
//! Every rule was measured against the pinned cargo-mutants 27.1.0.

use anyhow::{Context, Result, ensure};

use super::implementations_by_name;

#[test]
fn the_reader_inherits_an_inline_module_skip() -> Result<()> {
    // `visit_item_mod` returns early on `attrs_excluded`, so a skipped inline
    // module suppresses everything declared inside it. A reader that only
    // looked at top-level `impl` items would miss those methods entirely rather
    // than recording them as skipped, and would understate the exclusion.
    let source = "\
mod outer {
    #[mutants::skip]
    mod excluded {
        struct Inner;
        impl Inner {
            fn under_skipped_mod(&self) -> u32 { 1 }
            fn also_under_skipped_mod(&self) -> u32 { 2 }
        }
    }

    mod included {
        struct Kept;
        impl Kept {
            fn under_open_mod(&self) -> u32 { 3 }
        }
    }
}
";
    let by_name = implementations_by_name(source)?;
    for name in ["under_skipped_mod", "also_under_skipped_mod"] {
        let selection = by_name
            .get(name)
            .with_context(|| format!("the probe must declare `{name}`"))?;
        ensure!(
            (selection.selected, selection.skipped) == (0, 1),
            "`{name}` sits in a skipped inline module, so cargo-mutants never \
             reaches it and it must read as skipped; got {selection:?}"
        );
    }

    let kept = by_name
        .get("under_open_mod")
        .context("the probe must declare `under_open_mod`")?;
    ensure!(
        (kept.selected, kept.skipped) == (1, 0),
        "a sibling module without the attribute must stay selected, so the \
         exclusion above is inherited from the module rather than from the \
         nesting; got {kept:?}"
    );
    Ok(())
}

#[test]
fn the_reader_honours_the_tools_eligibility_rules() -> Result<()> {
    // Three methods produce no mutants without carrying any attribute: an
    // `unsafe fn`, a constructor named `new`, and a method whose body is empty.
    // All three were measured against the pinned tool; counting them as
    // selected would claim the ordinary lane judges code it never mutates.
    let source = "\
impl Demo {
    fn plain(&self) -> u32 { 1 }

    unsafe fn unsafe_method(&self) -> u32 { 2 }

    fn new() -> Self { Demo }

    fn empty_body(&self) {}
}
";
    let by_name = implementations_by_name(source)?;
    for name in ["unsafe_method", "new", "empty_body"] {
        let selection = by_name
            .get(name)
            .with_context(|| format!("the probe must declare `{name}`"))?;
        ensure!(
            (selection.selected, selection.skipped) == (0, 1),
            "`{name}` is ineligible for mutation — unsafe, a `new` constructor, \
             or an empty body — so it must read as skipped; got {selection:?}"
        );
    }

    let plain = by_name
        .get("plain")
        .context("the probe must declare `plain`")?;
    ensure!(
        (plain.selected, plain.skipped) == (1, 0),
        "an ordinary method must stay selected, so the exclusions above are \
         specific to the eligibility rules; got {plain:?}"
    );
    Ok(())
}

#[test]
fn the_reader_separates_selected_production_from_selected_kani() -> Result<()> {
    // A selected `cfg(kani)` implementation is not a production counterpart.
    // Counting it as one would let a name pass the counterpart check after its
    // real counterpart moved behind the Kani gate.
    let source = "\
impl Demo {
    #[cfg(not(kani))]
    fn paired(&self) -> u32 { 1 }

    #[cfg(kani)]
    fn paired(&self) -> u32 { 2 }

    #[cfg(kani)]
    fn proof_only(&self) -> u32 { 3 }
}
";
    let by_name = implementations_by_name(source)?;
    let paired = by_name
        .get("paired")
        .context("the probe must declare `paired`")?;
    ensure!(
        (paired.selected, paired.selected_production) == (2, 1),
        "a same-named pair of selected implementations must report one selected \
         production counterpart, not two; got {paired:?}"
    );

    let proof_only = by_name
        .get("proof_only")
        .context("the probe must declare `proof_only`")?;
    ensure!(
        (proof_only.selected, proof_only.selected_production) == (1, 0),
        "a Kani-only implementation is selected but is not a production \
         counterpart; got {proof_only:?}"
    );
    Ok(())
}

#[test]
fn the_reader_classifies_compound_kani_gates_by_what_they_permit() -> Result<()> {
    // A bare `kani` token scan would call every predicate that mentions `kani`
    // a gate, which misclassifies `cfg(any(kani, unix))`: `unix` alone can
    // satisfy it, so the item does compile in the ordinary lane. The
    // classification follows the boolean structure instead.
    let source = "\
impl Demo {
    #[cfg(kani)]
    fn bare_kani(&self) -> u32 { 1 }

    #[cfg(all(kani))]
    fn all_of_kani(&self) -> u32 { 2 }

    #[cfg(all(kani, unix))]
    fn all_kani_and_unix(&self) -> u32 { 3 }

    #[cfg(any(kani, unix))]
    fn any_kani_or_unix(&self) -> u32 { 4 }

    #[cfg(not(kani))]
    fn not_kani(&self) -> u32 { 5 }
}
";
    let by_name = implementations_by_name(source)?;
    for name in ["bare_kani", "all_of_kani", "all_kani_and_unix"] {
        let selection = by_name
            .get(name)
            .with_context(|| format!("the probe must declare `{name}`"))?;
        ensure!(
            (selection.selected, selection.selected_production) == (1, 0),
            "`{name}` cannot compile without `kani`, so it must not count as a \
             production implementation; got {selection:?}"
        );
    }

    for name in ["any_kani_or_unix", "not_kani"] {
        let selection = by_name
            .get(name)
            .with_context(|| format!("the probe must declare `{name}`"))?;
        ensure!(
            (selection.selected, selection.selected_production) == (1, 1),
            "`{name}` can compile without `kani`, so it is production code the \
             ordinary lane judges; got {selection:?}"
        );
    }
    Ok(())
}

#[test]
fn the_reader_inherits_an_enclosing_kani_gate() -> Result<()> {
    // `#[cfg(kani)] impl Demo { .. }` puts every method behind the gate, so
    // none of them is a production implementation even though no method carries
    // an attribute of its own. Reading only the method's attributes would count
    // them as production and let a real counterpart hiding behind an enclosing
    // gate pass the counterpart check.
    let source = "\
#[cfg(kani)]
impl Gated {
    fn under_kani_impl(&self) -> u32 { 1 }
}

impl Open {
    fn under_open_impl(&self) -> u32 { 2 }
}
";
    let by_name = implementations_by_name(source)?;
    let gated = by_name
        .get("under_kani_impl")
        .context("the probe must declare `under_kani_impl`")?;
    ensure!(
        (gated.selected, gated.selected_production) == (1, 0),
        "a method inside a `cfg(kani)` impl must not count as production; got \
         {gated:?}"
    );

    let open = by_name
        .get("under_open_impl")
        .context("the probe must declare `under_open_impl`")?;
    ensure!(
        (open.selected, open.selected_production) == (1, 1),
        "a method in an ungated impl must stay production; got {open:?}"
    );
    Ok(())
}
