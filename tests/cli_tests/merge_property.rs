//! Property-based tests for the layered CLI configuration merge invariants.
//!
//! This file holds the generative cases that the sibling merge properties do
//! not already reach. [`super::merge_precedence_proptests`] and
//! [`super::merge_targets_proptests`] own the layer-precedence and list-append
//! contracts across generated inputs, so those are deliberately not restated
//! here. What remains is:
//!
//! - the exact `jobs` acceptance boundary, which elsewhere appears only as the
//!   fixed examples `jobs = 0` and `jobs = 65`: four deterministic edge cases
//!   pin the decisive values, and a property samples the interior;
//! - file-supplied `cmds.build.targets` surviving the full ladder, where the
//!   sibling properties assert only that this field stays empty when the file
//!   layer configures `default_targets` instead.
//!
//! The `theme`/`no_emoji` and `spinner_mode`/`progress` cross-field rules an
//! earlier revision covered no longer exist. `CliConfig` replaced those paired
//! settings with the single `EmojiPolicy` and `ProgressPolicy` enums, so the
//! contradictory states became unrepresentable rather than rejected, and their
//! resolution is swept exhaustively by [`super::display_policy_domain`].
//! Rebuilding the old cases would test a validation path that the schema no
//! longer has.

use netsuke::cli::CliConfig;
use ortho_config::{MergeComposer, sanitize_value};
use proptest::prelude::*;
use rstest::rstest;
use serde_json::{Value, json};

/// Upper bound of the documented `jobs` range.
///
/// Mirrors the CLI contract rather than importing it: the value is asserted
/// here as the published boundary, so widening the production constant without
/// reviewing this contract fails the suite.
const MAX_JOBS: usize = 64;

/// Merge a single file layer over the supplied schema defaults.
///
/// Defaults are always pushed so every case starts from the same baseline.
/// Keeping the composition to the file layer isolates the field under test
/// from the environment and CLI layers, which the sibling properties drive.
///
/// The caller supplies `defaults` rather than sanitizing them here, so the
/// fallible step stays inside the test body where the test-only `expect`
/// exemption applies.
///
/// # Errors
///
/// Propagates an `OrthoError` when the generated layers do not merge cleanly.
fn merge_file_layer(defaults: Value, file_layer: Value) -> ortho_config::OrthoResult<CliConfig> {
    let mut composer = MergeComposer::new();
    composer.push_defaults(defaults);
    composer.push_file(file_layer, None);
    CliConfig::merge_from_layers(composer.layers())
}

/// A generated list of lower-case target names.
fn target_list(max: usize) -> impl Strategy<Value = Vec<String>> {
    proptest::collection::vec("[a-z]{2,6}", 0..=max)
}

/// The four decisive values of the documented `jobs` acceptance rule.
///
/// Each row names the boundary it pins, so a failure identifies the edge that
/// moved. The expected outcome is written alongside the value rather than
/// recomputed from `jobs == 0 || jobs > MAX_JOBS`, so a production change to
/// the predicate fails these cases instead of silently redefining them.
#[rstest]
#[case::zero_is_rejected(0, false)]
#[case::one_is_accepted(1, true)]
#[case::max_is_accepted(MAX_JOBS, true)]
#[case::over_max_is_rejected(MAX_JOBS + 1, false)]
fn jobs_boundaries_match_the_documented_rule(#[case] jobs: usize, #[case] accepted: bool) {
    let defaults = sanitize_value(&CliConfig::default()).expect("sanitizable defaults");
    let outcome = merge_file_layer(defaults, json!({ "jobs": jobs }));
    assert_eq!(
        outcome.is_ok(),
        accepted,
        "jobs={} outcome={:?}",
        jobs,
        outcome.err().map(|error| error.to_string())
    );
}

proptest! {
    /// `jobs` is accepted exactly within the inclusive range `1..=MAX_JOBS`.
    ///
    /// The domain sweeps from zero to twice the limit, so interior values from
    /// both the accepted and the rejected spans are sampled. The four decisive
    /// edges are pinned deterministically by
    /// [`jobs_boundaries_match_the_documented_rule`]; this sweep complements
    /// that rather than replacing it, because sampling alone cannot promise any
    /// particular value is reached in a given run.
    #[test]
    fn jobs_bounds_validation_matches_documented_rules(jobs in 0_usize..=(MAX_JOBS * 2)) {
        let defaults = sanitize_value(&CliConfig::default()).expect("sanitizable defaults");
        let outcome = merge_file_layer(defaults, json!({ "jobs": jobs }));
        let out_of_bounds = jobs == 0 || jobs > MAX_JOBS;
        prop_assert_eq!(
            outcome.is_err(),
            out_of_bounds,
            "jobs={} outcome={:?}",
            jobs,
            outcome.err().map(|error| error.to_string())
        );
    }

    /// File-supplied `cmds.build.targets` survive the merge ladder intact.
    ///
    /// `default_targets` is a compatibility alias that appends alongside the
    /// explicit subcommand field, so the two can drift apart if the alias
    /// machinery ever starts writing through to `cmds`. The sibling properties
    /// pin the opposite direction, asserting `cmds.build.targets` stays empty
    /// when only the alias is configured; this guards that a generated list
    /// supplied through the explicit field is passed through unchanged rather
    /// than dropped, reordered, or replaced by the alias.
    #[test]
    fn file_build_targets_survive_the_merge(targets in target_list(4)) {
        let defaults = sanitize_value(&CliConfig::default()).expect("sanitizable defaults");
        let merged =
            merge_file_layer(defaults, json!({ "cmds": { "build": { "targets": targets } } }))
                .expect("generated file layer must merge cleanly");
        prop_assert_eq!(merged.cmds.build.targets, targets);
    }
}
