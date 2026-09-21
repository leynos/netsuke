//! The checks a recorded benchmark run is held to.
//!
//! Split from the crate root to keep both readable: the root holds the
//! observable-behaviour cases, and this module holds the reusable machinery
//! they share — what a recorded pass must look like, and what a run of them
//! must satisfy. The per-variant contract lives in [`variant`].
//!
//! The properties stated over a range of starting states live in [`properties`],
//! because they are a different kind of claim from the ones here: these are
//! about what a particular run did, and those are about what every run must do.

#[path = "checks/order.rs"]
mod order;
#[path = "checks/properties.rs"]
mod properties;
#[path = "checks/variant.rs"]
mod variant;

use anyhow::{Context, Result, bail, ensure};
use test_support::build_tools::{
    BENCH_REPEATS, BENCH_SLUGS, CargoInvocation, DEFAULT_SLUG, MOLD_SLUG, MOLD_THREADS_SLUG,
    pinned_toolchain,
};
use variant::BenchVariant;

pub use order::{measured_slugs, order_records, order_varies_across_samples};

/// A cell holding a one-decimal duration, as `bench-build` formats them.
/// Timings are inherently unstable, so tests assert on shape, not value.
pub fn is_timing(cell: &str) -> bool {
    // Exactly one point, with digits either side. A looser check accepts `.`,
    // `..`, and `1.2.3`, none of which the benchmark can emit.
    let Some((whole, fraction)) = cell.split_once('.') else {
        return false;
    };
    !whole.is_empty()
        && !fraction.is_empty()
        && whole.chars().all(|c| c.is_ascii_digit())
        && fraction.chars().all(|c| c.is_ascii_digit())
}

pub const VARIANT_FLAGS: [(&str, &[&str]); 3] = [
    (DEFAULT_SLUG, &[]),
    (MOLD_SLUG, &["-Clink-arg=-fuse-ld=mold"]),
    (
        MOLD_THREADS_SLUG,
        &["-Zthreads=8", "-Clink-arg=-fuse-ld=mold"],
    ),
];

/// The rows a run off Linux must report, where `mold` does not exist.
///
/// The linker row goes entirely: there is no platform linker to compare `mold`
/// against, so a row for it would either be empty — a second measurement of the
/// baseline under a caption claiming otherwise — or would fail outright. The
/// threaded row survives and loses only the linker flag, which is why its
/// caption names the frontend as what it varies.
///
/// The baseline is the same row on both platforms, so it is written once.
pub const NON_LINUX_VARIANT_FLAGS: [(&str, &[&str]); 2] =
    [(DEFAULT_SLUG, &[]), (MOLD_THREADS_SLUG, &["-Zthreads=8"])];

/// The captions a run off Linux must print, in table order.
pub const NON_LINUX_CAPTIONS: [&str; 2] = [
    "| Default (platform linker) |",
    "| Platform linker, parallel frontend |",
];

/// Resolve every recorded pair to the variant it measured, hold each to that
/// variant's contract, and check the repeat counts.
///
/// The shared reading behind both entry points below. Both have to pair the
/// passes, name the variant each pair ran as, look that name up in a table and
/// check the pair against it, then count how often each variant appeared;
/// stating that twice invites the two readings to drift, and a drift here is a
/// check that silently stops describing the run it is given.
///
/// The table is a parameter rather than [`VARIANT_FLAGS`] read directly,
/// because the two platforms do not share one. Linux measures three rows and
/// every other host measures two, so a reading that could only consult the
/// Linux table could not describe a run off Linux at all. The table also
/// supplies the expected slugs, so there is no second list to keep in step
/// with it.
///
/// # Errors
///
/// Returns an error if the passes do not pair, if a pass runs outside a
/// variant directory, if a slug is not in the table, if a pass departs from
/// its variant's contract, or if a variant is measured the wrong number of
/// times.
fn checked_variants<'a>(
    invocations: &'a [CargoInvocation],
    table: &'a [(&'a str, &'a [&'a str])],
) -> Result<Vec<BenchVariant<'a>>> {
    let (pairs, rest) = invocations.as_chunks::<2>();
    ensure!(
        rest.is_empty(),
        "passes should come in pairs, got {} spare",
        rest.len()
    );

    let toolchain = pinned_toolchain()?;
    let variants: Vec<BenchVariant<'a>> = pairs
        .iter()
        .map(|pair| {
            // Drop the sample index the directory carries, so a variant
            // measured in two samples is checked against one expectation
            // rather than needing a row per sample.
            let (_, slug) = pair[0].target_dir().rsplit_once('/').with_context(|| {
                format!(
                    "pass should run in a variant directory, got `{}`",
                    pair[0].target_dir()
                )
            })?;
            let Some((label, flags)) = table.iter().find(|(known, _)| *known == slug) else {
                bail!("`{slug}` is not a variant this platform measures");
            };
            // The whole per-variant contract, not just the flags. A row is only
            // comparable with its neighbours if it also built the same binary
            // under the same toolchain with both wrappers cleared, and only the
            // flags differ between the tables, so the descriptor is reused
            // rather than restated here.
            let variant = BenchVariant::from_pair(label, pair, flags);
            variant.check(&toolchain)?;
            Ok(variant)
        })
        .collect::<Result<Vec<_>>>()?;

    // Every variant measured the same number of times. Otherwise a shuffle
    // could drop one from a sample and the table would still look plausible
    // while comparing unequal evidence.
    for (slug, _) in table {
        let seen = variants
            .iter()
            .filter(|variant| variant.label == *slug)
            .count();
        ensure!(
            seen == BENCH_REPEATS,
            "`{slug}` should be measured {BENCH_REPEATS} time(s), found {seen}"
        );
    }
    Ok(variants)
}

/// Hold every recorded pass to a variant table: each slug measured the right
/// number of times, each pass passing exactly that variant's flags.
///
/// The entry point for a run whose variant table is not the Linux one — off
/// Linux there is no `mold` row to measure.
///
/// # Errors
///
/// Returns an error if a pass runs outside a variant directory, if a slug is
/// not in the table, if a variant is measured the wrong number of times, or if
/// a pass's `RUSTFLAGS` are not exactly that variant's.
pub fn check_variant_flags(
    invocations: &[CargoInvocation],
    table: &[(&str, &[&str])],
) -> Result<()> {
    checked_variants(invocations, table).map(|_| ())
}

/// Check the recorded passes: their membership and pairing, each variant's own
/// contract, that variants measured separately, and where each pass sits
/// relative to the touch.
///
/// Nothing here depends on the order the variants ran in. The benchmark
/// shuffles them per sample precisely because a fixed order confounds the
/// variant with shared host state, so an assertion that pinned the order would
/// re-impose the defect the shuffle exists to remove. What must hold instead is
/// that every slug appears `2 * BENCH_REPEATS` times, and that each appearance
/// is a clean pass followed by an incremental one.
///
/// # Errors
///
/// Returns an error if the run measured the wrong number of passes, if a pass
/// departs from its variant's contract, if two variants shared a target
/// directory, or if a pass sits on the wrong side of the touch.
pub fn check_benchmark_invocations(
    invocations: &[CargoInvocation],
    baseline_mtime: i64,
) -> Result<()> {
    // Two builds — clean then incremental — for each variant in each sample.
    ensure!(
        invocations.len() == 2 * BENCH_SLUGS.len() * BENCH_REPEATS,
        "should measure two builds per variant per sample, recorded {}",
        invocations.len()
    );

    let variants = checked_variants(invocations, &VARIANT_FLAGS)?;

    // Pairwise rather than against the first alone: two accelerated variants
    // sharing a directory would warm each other's cache and understate the
    // second, which a check against the baseline would not catch. Compared
    // across samples too, so an accidental per-sample directory cannot hide.
    for (index, variant) in variants.iter().enumerate() {
        for other in variants.iter().skip(index + 1) {
            ensure!(
                variant.target_dir() != other.target_dir() || variant.label == other.label,
                "`{}` and `{}` share a directory, got `{}`",
                variant.label,
                other.label,
                variant.target_dir()
            );
        }
    }

    // `first()` rather than an index: the pair count was asserted above, but an
    // indexing panic here would report a slice bound rather than the missing
    // measurement the assertion is about.
    let (pairs, _) = invocations.as_chunks::<2>();
    let first = pairs
        .first()
        .context("the run should record at least one pair")?;
    check_touch_ordering(first, invocations, baseline_mtime)
}

/// Everything a whole recorded run must satisfy: the passes themselves, and the
/// order records it printed about them.
///
/// One entry point rather than two calls at each site, because the order check
/// reads the pairing this one establishes — it steps through the passes two at
/// a time. Splitting them across call sites invites a caller that does one and
/// not the other, which is exactly how the records went unchecked.
///
/// # Errors
///
/// Returns an error if any part of the recorded run departs from its contract.
pub fn check_recorded_run(
    stdout: &str,
    invocations: &[CargoInvocation],
    baseline_mtime: i64,
) -> Result<()> {
    check_benchmark_invocations(invocations, baseline_mtime)?;
    order::check_order_records(stdout, invocations, &BENCH_SLUGS)
}

/// Only the first pass of the whole run precedes any touch; the benchmark
/// touches the file between every variant's two passes, so every later pass
/// must see a newer timestamp. Comparing against a backdated baseline rather
/// than between passes keeps this free of filesystem timestamp granularity.
fn check_touch_ordering(
    first_pair: &[CargoInvocation],
    invocations: &[CargoInvocation],
    baseline_mtime: i64,
) -> Result<()> {
    let first = first_pair.first().context("expected a recorded pass")?;
    ensure!(
        first.touch_mtime() == Some(baseline_mtime),
        "the run's first clean pass should precede any touch, got {:?}",
        first.touch_mtime()
    );
    // Every pass after the run's very first follows a touch: the incremental
    // pass of the opening variant, and both passes of every variant after it.
    for (index, pass) in invocations.iter().enumerate().skip(1) {
        ensure!(
            pass.touch_mtime()
                .is_some_and(|mtime| mtime > baseline_mtime),
            "pass {index} should follow a touch, got {:?}",
            pass.touch_mtime()
        );
    }
    Ok(())
}
