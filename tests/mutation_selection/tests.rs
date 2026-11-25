//! Probes for the exclusion rules `cargo-mutants` applies.
//!
//! Each case is a source fragment fed to
//! [`implementations_by_name`](super::implementations_by_name), so it states
//! what the reader must decide rather than what the repository happens to
//! contain. The graph module's own selection is asserted in
//! `tests/mutation_selection_contract_tests.rs`; these are about the reader.
//!
//! Every rule here was measured against the pinned cargo-mutants 27.1.0. Two of
//! them are narrower than a first reading of the tool's source suggests, and
//! the probes record the narrow form because the broad form was falsified:
//!
//! - a `cfg_attr` condition must be a bare path, so `any()` is rejected and the
//!   skip inside it is silently dropped;
//! - only a *top-level* `test` in a `cfg` excludes a node, so
//!   `#[cfg(all(test, unix))]` is still mutated.

use anyhow::{Context, Result, ensure};

use super::implementations_by_name;

#[test]
fn the_reader_ignores_a_skip_that_cargo_mutants_would_drop() -> Result<()> {
    // `cfg_attr(any(), mutants::skip)` is not honoured by cargo-mutants: its
    // reader accepts only a bare-path condition, so a skip written inside a
    // parenthesized one is silently discarded. A contract that counted it as a
    // skip would certify an exclusion the pinned tool does not perform.
    let source = "\
impl Demo {
#[cfg_attr(any(), mutants::skip)]
fn parenthesized_condition(&self) -> u32 { 1 }

#[cfg_attr(test, mutants::skip)]
fn bare_path_condition(&self) -> u32 { 2 }
}
";
    let by_name = implementations_by_name(source)?;
    let dropped = by_name
        .get("parenthesized_condition")
        .context("the probe must declare `parenthesized_condition`")?;
    ensure!(
        (dropped.selected, dropped.skipped) == (1, 0),
        "a skip under a parenthesized cfg_attr condition must read as selected, \
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
fn the_reader_honours_the_parenthesized_skip_spelling() -> Result<()> {
    // cargo-mutants matches the attribute *path*, so `#[mutants::skip()]` and
    // any argument list are honoured; the `mutants` crate's attribute ignores
    // its arguments. A reader that only recognized the bare `Meta::Path` form
    // would miss this spelling and report a skipped item as selected.
    let source = "\
#[mutants::skip()]
impl Demo {
fn under_parenthesized_skip(&self) -> u32 { 1 }
fn also_under_parenthesized_skip(&self) -> u32 { 2 }
}
";
    let by_name = implementations_by_name(source)?;
    for name in ["under_parenthesized_skip", "also_under_parenthesized_skip"] {
        let selection = by_name
            .get(name)
            .with_context(|| format!("the probe must declare `{name}`"))?;
        ensure!(
            (selection.selected, selection.skipped) == (0, 1),
            "`{name}` sits in a block marked `#[mutants::skip()]`, which \
         cargo-mutants honours, so it must read as skipped; got {selection:?}"
        );
    }
    Ok(())
}

#[test]
fn the_reader_honours_an_enclosing_impl_skip() -> Result<()> {
    // cargo-mutants returns from `visit_item_impl` before it reaches the
    // methods, so a skip on the block suppresses every method in it even though
    // no method carries an attribute. A reader that inspected only the method's
    // own attributes would call these selected and disagree with the tool it
    // claims to model.
    let source = "\
#[mutants::skip]
impl Demo {
fn under_skipped_impl(&self) -> u32 { 1 }
}
";
    let by_name = implementations_by_name(source)?;
    let enclosed = by_name
        .get("under_skipped_impl")
        .context("the probe must declare `under_skipped_impl`")?;
    ensure!(
        (enclosed.selected, enclosed.skipped) == (0, 1),
        "a method inside a skipped impl block must read as skipped, because \
     cargo-mutants never visits it; got {enclosed:?}"
    );
    Ok(())
}

#[test]
fn the_reader_excludes_bare_cfg_test_like_the_tool_does() -> Result<()> {
    // `#[mutants::skip]` is not the only exclusion cargo-mutants applies: its
    // `attrs_excluded` also matches a `cfg` whose own condition is exactly
    // `test`, at both `impl` and method scope. Counting such an item as selected
    // would claim the ordinary lane judges code the tool never mutates.
    //
    // The rule is deliberately narrower than "any cfg mentioning test". The
    // tool reads the cfg's *top-level* conditions, so a `test` nested inside
    // `all(..)` or `any(..)` is not seen. Measured against cargo-mutants
    // 27.1.0: `#[cfg(test)]` yields no mutant, while `#[cfg(all(test, unix))]`
    // and `#[cfg(any(test, feature = "x"))]` both still do. Modelling the
    // broader rule would mark mutants as excluded that the tool would report.
    let source = "\
#[cfg(test)]
impl TestOnly {
fn under_cfg_test_impl(&self) -> u32 { 1 }
}

impl Demo {
fn plain(&self) -> u32 { 2 }

#[cfg(test)]
fn under_cfg_test_method(&self) -> u32 { 3 }

#[cfg(all(test, unix))]
fn under_nested_test_cfg(&self) -> u32 { 4 }
}
";
    let by_name = implementations_by_name(source)?;
    for name in ["under_cfg_test_impl", "under_cfg_test_method"] {
        let selection = by_name
            .get(name)
            .with_context(|| format!("the probe must declare `{name}`"))?;
        ensure!(
            (selection.selected, selection.skipped) == (0, 1),
            "`{name}` carries a bare `#[cfg(test)]` and produces no mutants, so \
         it must read as excluded rather than selected; got {selection:?}"
        );
    }

    for name in ["plain", "under_nested_test_cfg"] {
        let selection = by_name
            .get(name)
            .with_context(|| format!("the probe must declare `{name}`"))?;
        ensure!(
            (selection.selected, selection.skipped) == (1, 0),
            "`{name}` still produces mutants — a `test` nested in `all(..)` is \
         not seen by the tool's top-level scan — so it must stay selected; \
         got {selection:?}"
        );
    }
    Ok(())
}

#[test]
fn the_reader_honours_a_file_level_skip() -> Result<()> {
    // A file-level `#![mutants::skip]` is an inner attribute on the file, and
    // the tool returns from `visit_file` before walking any item. Nothing
    // inside such a file produces mutants.
    let source = "\
#![mutants::skip]

impl Demo {
fn in_a_skipped_file(&self) -> u32 { 1 }
}
";
    let by_name = implementations_by_name(source)?;
    let enclosed = by_name
        .get("in_a_skipped_file")
        .context("the probe must declare `in_a_skipped_file`")?;
    ensure!(
        (enclosed.selected, enclosed.skipped) == (0, 1),
        "a method in a file carrying an inner mutants::skip must read as \
     skipped, because cargo-mutants never walks it; got {enclosed:?}"
    );
    Ok(())
}

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
