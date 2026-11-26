//! Probes for how a `cfg` predicate decides whether code is proof-only.
//!
//! The model answers one question — can this hold with `kani` false? — and the
//! cases here are the shapes that question has to get right: bare and compound
//! `kani`, a negated atom under De Morgan, a raw identifier spelling, and a
//! predicate naming two values of an option Rust assigns exactly one value.
//! Split from `scope_tests` to keep each file inside the repository's 400-line
//! limit.

use anyhow::{Context, Result, ensure};

use super::implementations_by_name;

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

    #[cfg(not(not(kani)))]
    fn double_negation(&self) -> u32 { 6 }

    #[cfg(not(any(kani, unix)))]
    fn not_any_kani_or_unix(&self) -> u32 { 7 }

    #[cfg(not(unix))]
    fn not_unix(&self) -> u32 { 8 }

    #[cfg(r#kani)]
    fn raw_kani(&self) -> u32 { 9 }

    #[cfg(all(not(kani), target_os = \"linux\", target_os = \"windows\"))]
    fn impossible_target(&self) -> u32 { 10 }
}
";
    let by_name = implementations_by_name(source)?;
    for name in [
        "Demo::bare_kani",
        "Demo::all_of_kani",
        "Demo::all_kani_and_unix",
    ] {
        let selection = by_name
            .get(name)
            .with_context(|| format!("the probe must declare `{name}`"))?;
        ensure!(
            (selection.selected, selection.selected_production) == (1, 0),
            "`{name}` cannot compile without `kani`, so it must not count as a \
             production implementation; got {selection:?}"
        );
    }

    Ok(())
}

#[test]
fn the_reader_refuses_impossible_built_in_assignments() -> Result<()> {
    let source = "\
impl Demo {
    #[cfg(all(not(kani), target_os = \"linux\", target_os = \"windows\"))]
    fn impossible_target(&self) -> u32 { 10 }

    #[cfg(all(not(kani), feature = \"a\", feature = \"b\"))]
    fn two_features(&self) -> u32 { 11 }

    #[cfg(all(not(kani), target_family = \"unix\", target_family = \"wasm\"))]
    fn two_families(&self) -> u32 { 12 }

    #[cfg(all(not(kani), unix, not(target_family = \"unix\")))]
    fn derived_mix(&self) -> u32 { 13 }
}
";
    let by_name = implementations_by_name(source)?;
    // Rust assigns `target_os` exactly one value, so a predicate naming two of
    // them describes no target. Searching independent assignments for it would
    // answer about an impossible world, so the model must decline rather than
    // report a verdict.
    let impossible = by_name
        .get("Demo::impossible_target")
        .context("the probe must declare `Demo::impossible_target`")?;
    ensure!(
        impossible.undecidable == 1,
        "a predicate naming two values of a single-valued built-in option must \
         be reported undecidable, not classified; got {impossible:?}"
    );

    // `target_family` is multi-valued: the reference lists a target with both
    // `"unix"` and `"wasm"`, so two families are a real configuration and the
    // predicate is ordinary code rather than an impossible one.
    let families = by_name
        .get("Demo::two_families")
        .context("the probe must declare `Demo::two_families`")?;
    ensure!(
        (families.undecidable, families.selected_production) == (0, 1),
        "two `target_family` values are a valid configuration, so this is \
         production code rather than undecidable; got {families:?}"
    );

    // `feature` is multi-valued, so two distinct features are a real
    // configuration. Asserting it keeps that distinction from being folded into
    // the single-valued guard without any test noticing.
    let features = by_name
        .get("Demo::two_features")
        .context("the probe must declare `Demo::two_features`")?;
    ensure!(
        (features.undecidable, features.selected_production) == (0, 1),
        "two `feature` values are a valid configuration, so this is production \
         code rather than undecidable; got {features:?}"
    );

    // `unix` is defined as exactly `target_family = "unix"`, so the two are not
    // independent atoms. Declining is sound; searching them separately would
    // accept a target that cannot exist.
    let derived = by_name
        .get("Demo::derived_mix")
        .context("the probe must declare `Demo::derived_mix`")?;
    ensure!(
        derived.undecidable == 1,
        "a predicate mixing a derived family name with `target_family` must be \
         declined rather than searched independently; got {derived:?}"
    );

    Ok(())
}

#[test]
fn the_reader_handles_negation_through_the_atoms() -> Result<()> {
    // `not(not(kani))` is `kani` and is still a gate. A negated *non-Kani* atom
    // is not: `not(unix)` holds whenever `unix` is false, and
    // `not(any(kani, unix))` is `all(not(kani), not(unix))`, which holds when
    // both are false. A raw spelling is the same identifier to rustc, so
    // `r#kani` pins the atom exactly as `kani` does.
    let source = "\
impl Demo {
    #[cfg(r#kani)]
    fn raw_kani(&self) -> u32 { 9 }

    #[cfg(not(not(kani)))]
    fn double_negation(&self) -> u32 { 10 }

    #[cfg(any(kani, unix))]
    fn any_kani_or_unix(&self) -> u32 { 11 }

    #[cfg(not(kani))]
    fn not_kani(&self) -> u32 { 12 }

    #[cfg(not(any(kani, unix)))]
    fn not_any_kani_or_unix(&self) -> u32 { 13 }

    #[cfg(not(unix))]
    fn not_unix(&self) -> u32 { 14 }
}
";
    let by_name = implementations_by_name(source)?;
    // A raw spelling is the same identifier to rustc: `r#kani` needs no
    // separate check-cfg declaration, so it must pin the atom too.
    let raw = by_name
        .get("Demo::raw_kani")
        .context("the probe must declare `Demo::raw_kani`")?;
    ensure!(
        (raw.selected, raw.selected_production) == (1, 0),
        "`#[cfg(r#kani)]` is the same atom as `#[cfg(kani)]` and cannot \
         compile without it, so it must not count as production; got {raw:?}"
    );

    // `not(not(kani))` is `kani`, so it is still a gate.
    let doubled = by_name
        .get("Demo::double_negation")
        .context("the probe must declare `double_negation`")?;
    ensure!(
        (doubled.selected, doubled.selected_production) == (1, 0),
        "`not(not(kani))` is `kani` and cannot compile without it, so it must \
         not count as production; got {doubled:?}"
    );

    // A negated *non-Kani* atom is not a gate: `not(unix)` holds whenever
    // `unix` is false, and `not(any(kani, unix))` is
    // `all(not(kani), not(unix))`, which holds when both are false. Both are
    // therefore satisfiable without Kani and are production code.
    for name in [
        "Demo::any_kani_or_unix",
        "Demo::not_kani",
        "Demo::not_any_kani_or_unix",
        "Demo::not_unix",
    ] {
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
