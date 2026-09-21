//! The cases that hold the variant *order* to its contract.
//!
//! Split from the crate root because they are a different claim from the ones
//! there: those establish that each variant was measured properly, and these
//! establish which order the variants were measured in and what decides it.
//! Keeping them together also keeps the root inside the 400-line limit.
//!
//! Three things are checked here. That the order is drawn afresh per sample
//! rather than walked straight through, that the draw is reproducible from the
//! seed the run prints, and that a repeat count which cannot produce comparable
//! evidence is refused rather than quietly measured.

use anyhow::{Context, Result, ensure};
use rstest::rstest;
use test_support::build_tools::{BenchFixture, BuildScenario, MOLD_SLUG, MakeInvocation, combined};

use crate::checks::{
    NON_LINUX_CAPTIONS, NON_LINUX_VARIANT_FLAGS, check_variant_flags, measured_slugs,
    order_records, order_varies_across_samples,
};

/// A `bench-build` invocation against the fake Cargo, ready for extra
/// environment. Every case here runs the same target and differs only in what
/// it sets around it.
fn bench_invocation(scenario: &BuildScenario, fixture: &BenchFixture) -> MakeInvocation {
    MakeInvocation::new("bench-build")
        .variable("CARGO", scenario.cargo().executable())
        .environment("BENCH_ROOT", &fixture.root)
        .environment("BENCH_TOUCH_FILE", &fixture.touch_file)
}

/// The variants really are shuffled, not merely printed as though they were.
///
/// Nothing else in this suite can tell those two apart. Every structural
/// assertion holds just as well for a fixed order, so a script that quietly
/// walked its variant list straight through would pass the lot while
/// re-introducing the defect the shuffle exists to remove — a table whose rows
/// are confounded with whoever else was on the host when each one ran.
///
/// Detecting it needs samples that disagree, which accumulates only with
/// samples: two draws of three variants coincide half the time under a true
/// shuffle. Twenty samples all agreeing is `6^-20`, about one run in 3.7
/// quadrillion, so the assertion is not one a correct implementation can trip.
///
/// Raising the repeat count is what buys the samples. Each one is a pair of
/// builds against a fake `cargo`, so the cost is process spawns rather than
/// compiles, and the whole case still finishes in well under a second.
#[test]
fn the_variant_order_is_actually_shuffled() -> Result<()> {
    // Small enough to stay cheap, large enough that agreement is implausible.
    // The bound is `permutations(BENCH_SLUGS.len()) ^ samples`, which is why
    // the count is asserted against the draw rather than left implicit.
    const SAMPLES: &str = "20";

    let scenario = BuildScenario::prepare()?;
    let fixture = BenchFixture::prepare(&scenario)?;
    let invocation = bench_invocation(&scenario, &fixture).environment("BENCH_REPEATS", SAMPLES);
    let output = scenario.sandbox().run_make(&invocation)?;

    ensure!(
        output.status.success(),
        "make bench-build should succeed, got `{}`",
        combined(&output)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let records = order_records(&stdout)?;
    let expected: usize = SAMPLES.parse().context("the repeat count is a number")?;
    ensure!(
        records.samples.len() == expected,
        "the run should print one record per sample, got {} for {expected}",
        records.samples.len()
    );

    order_varies_across_samples(&records)
}

/// Off Linux the linker row is gone, and the threaded row varies only the
/// frontend.
///
/// `mold` ships for Linux alone, so the three platforms do not share one
/// variant table and the two-row shape is the promise that the other two are
/// honoured. Faking `uname` is what makes it checkable from Linux — the branch
/// is reachable on hardware CI does not have, which is exactly where an
/// untested fallback rots. The scripts' own `is_linux` reads the same command,
/// so the fake reaches both the Makefile's `STANDARD_RUSTFLAGS` and the
/// script's variant selection.
///
/// The table is where this is observable: a `mold` row here would either
/// measure the baseline a second time under a caption claiming otherwise, or
/// fail outright for want of a linker that does not exist. Either way the run
/// would satisfy the Linux expectations while reporting a row that means
/// nothing.
#[test]
fn a_non_linux_host_reports_two_rows_and_varies_only_the_frontend() -> Result<()> {
    let scenario = BuildScenario::prepare()?;
    let sandbox = scenario.sandbox();
    let fixture = BenchFixture::prepare(&scenario)?;
    sandbox.write_fake(&sandbox.bin(), "uname", "echo Darwin")?;

    let output = sandbox.run_make(&bench_invocation(&scenario, &fixture))?;
    let stdout = String::from_utf8_lossy(&output.stdout);

    ensure!(
        output.status.success(),
        "make bench-build should succeed off Linux, got `{}`",
        combined(&output)
    );
    for caption in NON_LINUX_CAPTIONS {
        ensure!(
            stdout.contains(caption),
            "the table should carry the `{caption}` row, got `{stdout}`"
        );
    }
    ensure!(
        !stdout.contains("| `mold` |"),
        "a host without mold should report no linker row, got `{stdout}`"
    );

    // The invocations are the sharper check: a table can print any caption,
    // and what distinguishes the rows is the flags that reached Cargo.
    let invocations = scenario.cargo().invocations()?;
    let measured = measured_slugs(&invocations)?;
    ensure!(
        !measured.iter().any(|slug| slug == MOLD_SLUG),
        "the linker row should not be measured off Linux, got `{measured:?}`"
    );
    check_variant_flags(&invocations, &NON_LINUX_VARIANT_FLAGS)
}

/// One seed, one order: the draw is an input rather than ambient state.
///
/// The shuffle is part of the measurement, so a table nobody can reproduce is
/// a table nobody can check. The seed is what makes it reproducible, and the
/// run prints the one it used so a reader of the output can replay it.
///
/// Twenty samples is what makes this decisive in both directions. A script
/// that read the ambient generator and ignored the seed would still draw a
/// valid permutation every time, and the two runs would agree only with
/// probability `6^-20`; a script that honours the seed agrees always. One or
/// two samples would coincide often enough to prove nothing either way.
#[test]
fn one_seed_gives_one_order() -> Result<()> {
    const SAMPLES: &str = "20";
    const SEED: &str = "20260921";

    let mut drawn = Vec::new();
    for _ in 0..2 {
        let scenario = BuildScenario::prepare()?;
        let fixture = BenchFixture::prepare(&scenario)?;
        let invocation = bench_invocation(&scenario, &fixture)
            .environment("BENCH_REPEATS", SAMPLES)
            .environment("BENCH_SEED", SEED);
        let output = scenario.sandbox().run_make(&invocation)?;
        ensure!(
            output.status.success(),
            "a seeded run should succeed, got `{}`",
            combined(&output)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        ensure!(
            stdout.contains(&format!("order seed: {SEED}")),
            "the run should print the seed it drew from, got `{stdout}`"
        );
        drawn.push(order_records(&stdout)?.measured);
    }

    let (first, second) = (
        drawn.first().context("the first run recorded an order")?,
        drawn.get(1).context("the second run recorded an order")?,
    );
    ensure!(
        first == second,
        "the same seed should replay the same order, got {first:?} against {second:?}"
    );
    Ok(())
}

/// An unseeded run still prints a seed, so its table can be replayed later.
///
/// Without this the seed would be a facility nobody could use after the fact:
/// the interesting run is nearly always the one somebody already took, and a
/// seed that is only observable when supplied explains nothing about it.
#[test]
fn an_unseeded_run_reports_the_seed_it_drew() -> Result<()> {
    let scenario = BuildScenario::prepare()?;
    let fixture = BenchFixture::prepare(&scenario)?;
    let output = scenario
        .sandbox()
        .run_make(&bench_invocation(&scenario, &fixture))?;
    ensure!(
        output.status.success(),
        "an unseeded run should succeed, got `{}`",
        combined(&output)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout
        .lines()
        .find_map(|line| line.trim().strip_prefix("order seed:"))
        .context("an unseeded run should still report its seed")?;
    ensure!(
        line.trim().chars().all(|c| c.is_ascii_digit()) && !line.trim().is_empty(),
        "the reported seed should be a number to pass back, got `{line}`"
    );
    Ok(())
}

/// A repeat count that cannot produce comparable evidence is refused.
///
/// Nought prints an empty table and exits nought; one prints a table that is
/// indistinguishable in shape from a valid one while carrying exactly the
/// single-sample bias the repeats exist to spread. Neither failure is visible
/// in the output somebody pastes into the guide, which is why the script
/// refuses rather than clamps. A non-numeric value is refused for the same
/// reason: the arithmetic would otherwise treat it as nought. An empty value
/// is not among these: the script's `:-` default has already replaced it, as
/// it has for every other `BENCH_` variable.
#[rstest]
#[case::none("0")]
#[case::single("1")]
#[case::not_a_number("two")]
fn an_unusable_repeat_count_is_refused(#[case] repeats: &str) -> Result<()> {
    let scenario = BuildScenario::prepare()?;
    let fixture = BenchFixture::prepare(&scenario)?;
    let invocation = bench_invocation(&scenario, &fixture).environment("BENCH_REPEATS", repeats);
    let output = scenario.sandbox().run_make(&invocation)?;

    ensure!(
        !output.status.success(),
        "`BENCH_REPEATS={repeats}` should fail the run, got `{}`",
        combined(&output)
    );
    let text = combined(&output);
    ensure!(
        text.contains("BENCH_REPEATS must be a whole number of at least 2"),
        "the refusal should name the requirement, got `{text}`"
    );
    // Nothing measured: a refusal that had already built something would have
    // spent the time it exists to save, and left a partial table behind.
    ensure!(
        scenario.cargo().invocations()?.is_empty(),
        "a refused run should measure nothing"
    );
    Ok(())
}

/// A seed that is not a number is refused rather than silently read as nought.
///
/// Bash's arithmetic treats an unparsable value as nought, so a typo would
/// pin every run to one order while the printed record claimed the typo was
/// the seed — reproducible, and reproducibly wrong.
#[test]
fn a_seed_that_is_not_a_number_is_refused() -> Result<()> {
    let scenario = BuildScenario::prepare()?;
    let fixture = BenchFixture::prepare(&scenario)?;
    let invocation = bench_invocation(&scenario, &fixture).environment("BENCH_SEED", "today");
    let output = scenario.sandbox().run_make(&invocation)?;

    ensure!(
        !output.status.success(),
        "a non-numeric seed should fail the run, got `{}`",
        combined(&output)
    );
    let text = combined(&output);
    ensure!(
        text.contains("BENCH_SEED must be a whole number"),
        "the refusal should name the requirement, got `{text}`"
    );
    Ok(())
}
