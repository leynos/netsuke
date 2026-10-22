//! Regression guard for `rstest-bdd` step-result error propagation.
//!
//! Before 0.6.0, a step returning `Err` from a local alias of `Result<T, E>`
//! had its error boxed as an unused payload: the scenario stayed green. 0.6.0
//! classifies unhinted non-unit returns by their concrete type, so the alias
//! propagates the error and the scenario fails. That correction is the reason
//! this migration could surface a latent defect, and it needs a test that
//! fails if it ever regresses.
//!
//! Every other scenario in this repository walks a green path whose steps
//! return `Ok`, so none of them would detect such a regression. The scenario
//! below is deliberately driven to failure and asserts the failure happens.
//!
//! This test is self-contained — its steps, its feature file and its scenario
//! all live here. The feature lives in `tests/features_step_results/` rather
//! than `tests/features/` because `scenarios!` in `tests/bdd_tests.rs` sweeps
//! the latter directory and would otherwise collect this failing scenario as
//! an ordinary one expected to pass.

use rstest_bdd_macros::{given, scenario, then};

/// Fails unconditionally, with a marker the scenario assertion matches on.
///
/// The return type is written as the unhinted alias `anyhow::Result<()>`
/// deliberately: the behaviour under test is how the macro classifies an
/// *unhinted* non-unit return by its concrete type. Annotating it some other
/// way would exercise a different path and could pass while this one is
/// broken.
///
/// No `#[expect(clippy::unnecessary_wraps)]` is needed here, unlike the rest
/// of the step suite: these steps always return `Err`, so they are genuinely
/// fallible and Clippy does not suggest dropping the `Result`.
#[given("a step that always returns an error")]
fn always_fails() -> anyhow::Result<()> {
    anyhow::bail!("deliberate step error for propagation regression test")
}

/// Fails if it ever runs, proving the earlier error short-circuited the
/// scenario rather than being swallowed and execution continuing.
#[then("this step must never run")]
fn must_never_run() -> anyhow::Result<()> {
    anyhow::bail!("trailing step ran: the earlier error was not propagated")
}

/// Assert that a failing step fails its scenario.
///
/// This is a two-sided oracle. It passes only when the step's `Err` reaches
/// the generated step loop and panics there. If propagation regressed the way
/// it did before 0.6.0, the error would be swallowed, the second step would
/// run, and this test would fail on the *other* message — so the assertion
/// cannot be satisfied by accident.
#[scenario(
    path = "tests/features_step_results/step_error_propagation.feature",
    name = "An error returned by a step fails its scenario"
)]
#[should_panic(expected = "deliberate step error for propagation regression test")]
fn step_error_fails_its_scenario() {}
