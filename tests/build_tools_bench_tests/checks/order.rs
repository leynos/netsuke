//! The order records a benchmark run prints, and the contract they must meet.
//!
//! The script shuffles the variants afresh for each sample and prints the order
//! it drew, because that shuffle is not reconstructible after the fact: a table
//! whose rows disagree with an earlier run cannot otherwise be told apart from
//! one whose variant order differed.
//!
//! A record nothing reads back is a guarantee nothing holds. The row counts and
//! the timing cells are identical whether the run shuffled or not, and whether
//! it printed the records or dropped them, so those assertions cannot notice
//! either regression. These read the record back and hold it against what the
//! run actually executed.

use anyhow::{Context, Result, ensure};
use test_support::build_tools::CargoInvocation;

/// Prefix of a per-sample record, before the sample number.
const SAMPLE_PREFIX: &str = "order sample ";
/// Prefix of the run's whole measured sequence.
const MEASURED_PREFIX: &str = "order measured:";

/// The order records one run printed.
pub struct OrderRecords {
    /// One entry per sample, holding that sample's slugs in the order drawn.
    pub samples: Vec<Vec<String>>,
    /// The run's whole measured sequence, as the script reported it.
    pub measured: Vec<String>,
}

/// Read the order records out of a run's standard output.
///
/// # Errors
///
/// Returns an error if a record is malformed, or if the measured record is
/// absent — which is the case a run that dropped the record would produce.
pub fn order_records(stdout: &str) -> Result<OrderRecords> {
    let mut samples = Vec::new();
    let mut measured = None;
    for raw in stdout.lines() {
        let line = raw.trim_end();
        if let Some(rest) = line.strip_prefix(SAMPLE_PREFIX) {
            let (_, drawn) = rest.split_once(':').with_context(|| {
                format!("a sample record should carry its drawn order, got `{line}`")
            })?;
            samples.push(words(drawn));
        } else if let Some(rest) = line.strip_prefix(MEASURED_PREFIX) {
            measured = Some(words(rest));
        }
    }
    Ok(OrderRecords {
        samples,
        measured: measured.context("the run should print an `order measured` record")?,
    })
}

/// Split a record's payload into slugs.
fn words(text: &str) -> Vec<String> {
    text.split_whitespace().map(str::to_owned).collect()
}

/// The slugs the recorded invocations actually ran, in execution order.
///
/// Derived from each pass's target directory rather than from anything the
/// script printed, so this is an independent witness: the printed order is a
/// claim, and this is what happened. Every variant measures in a directory
/// named for its slug, which is what makes the log readable this way.
///
/// # Errors
///
/// Returns an error if a recorded pass ran outside a variant directory.
pub fn measured_slugs(invocations: &[CargoInvocation]) -> Result<Vec<String>> {
    invocations
        .iter()
        .step_by(2)
        .map(|pass| {
            let dir = pass.target_dir();
            let (_, slug) = dir.rsplit_once('/').with_context(|| {
                format!("a pass should run in a variant directory, got `{dir}`")
            })?;
            anyhow::Ok(slug.to_owned())
        })
        .collect()
}

/// Hold a run's printed order to three things: that every expected variant is
/// drawn exactly once per sample, that the measured record repeats the samples
/// in sequence, and that the whole thing matches the invocations that ran.
///
/// `invocations` must already have passed [`check_benchmark_invocations`], whose
/// pairing assertion is what makes every second pass a variant's clean one.
///
/// Deliberately says nothing about *whether* the order was shuffled. Every
/// assertion here holds for a fixed order too, because a fixed order is itself a
/// permutation and the record would report it faithfully. Only
/// [`order_varies_across_samples`] can tell the two apart, and it needs more
/// samples than a default run takes, so it is not folded in here.
///
/// # Errors
///
/// Returns an error if a sample is not a permutation of `expected`, or if the
/// records disagree with each other or with the run.
pub fn check_order_records(
    stdout: &str,
    invocations: &[CargoInvocation],
    expected: &[&str],
) -> Result<()> {
    let records = order_records(stdout)?;
    ensure!(
        !records.samples.is_empty(),
        "the run should print an `order sample` record for every sample it measured"
    );

    for (index, sample) in records.samples.iter().enumerate() {
        let number = index + 1;
        // Equal length plus every expected slug present *is* a permutation:
        // there is no room left for a repeat or an interloper.
        ensure!(
            sample.len() == expected.len(),
            "sample {number} should draw {} variant(s), got {sample:?}",
            expected.len()
        );
        for slug in expected {
            ensure!(
                sample.iter().any(|drawn| drawn == slug),
                "sample {number} should draw `{slug}`, got {sample:?}"
            );
        }
    }

    let drawn: Vec<&String> = records.samples.iter().flatten().collect();
    ensure!(
        records.measured.iter().collect::<Vec<_>>() == drawn,
        "the measured record should repeat every sample in order, got {:?} against {drawn:?}",
        records.measured
    );
    let ran = measured_slugs(invocations)?;
    ensure!(
        records.measured == ran,
        "the order printed should be the order that ran, got {:?} against {ran:?}",
        records.measured
    );
    Ok(())
}

/// Whether the run drew more than one distinct order: the only evidence that
/// the variants were shuffled rather than measured in a fixed sequence.
///
/// A permutation check cannot establish this. The declared order is itself a
/// permutation, so a script that dropped the shuffle and walked its list
/// straight through satisfies every structural assertion — and would then let
/// the variant order confound the table with host state, which is the defect
/// the shuffle exists to remove. What distinguishes the two is disagreement
/// between samples, and that only accumulates with samples: two draws of three
/// variants coincide half the time even under a true shuffle.
///
/// So the caller must supply enough samples for agreement to be implausible,
/// and say what the bound is. `order_varies_across_samples` does the first, and
/// the test that calls it states the second.
///
/// # Errors
///
/// Returns an error if the run printed no samples, or if every sample drew the
/// same order.
pub fn order_varies_across_samples(records: &OrderRecords) -> Result<()> {
    let Some(first) = records.samples.first() else {
        return Err(anyhow::anyhow!(
            "the run should print at least one `order sample` record"
        ));
    };
    ensure!(
        records.samples.iter().any(|sample| sample != first),
        "every one of the {} samples drew {:?}; the variants are not being shuffled, \
         and a fixed order lets host state confound the table",
        records.samples.len(),
        first
    );
    Ok(())
}
