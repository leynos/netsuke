//! Contract for the `test-kani-mutations` gate the `kani-smoke` lane runs.
//!
//! The gate is two Nextest invocations rather than one: the cold prewarm shares
//! its invocation with the negative check, and the patch sweep follows against
//! the target those two warmed. Both halves are asserted together because either
//! one alone leaves the gate green while proving less than it claims — a sweep
//! that silently ran first pays the dependency build, and a prewarm without the
//! negative check never shows the compile path can fail.
//!
//! Ordering is asserted through the Nextest profile's `priority`, which is the
//! only thing that puts the prewarm ahead of the negative check inside their
//! shared `nested-cargo-builds` slot. The parent `makefile_test_target` module
//! supplies the repository-file and recipe-lookup helpers.

use super::{read_repo_file, target_recipe};
use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use toml::Value;

/// Verify `test-kani-mutations` selects every ignored compile test.
///
/// Both commands must select the ignored test binary, carry the gate flags, and
/// keep the cold prewarm before the patch sweep. Otherwise nextest can skip the
/// actual compile work while the target exits successfully. The prewarm's own
/// invocation also selects the negative test, which compiles the same sandbox
/// and so reuses the target the prewarm has just warmed.
#[test]
fn behavioural_kani_mutation_target_selects_every_ignored_compile_test() -> Result<()> {
    let makefile = read_repo_file(Utf8Path::new("Makefile"))?;
    let recipe = target_recipe(&makefile, "test-kani-mutations")
        .context("Makefile should declare a test-kani-mutations target")?;
    // Match the command itself rather than any line that mentions it: a
    // comment or an `echo` describing the gate must not be mistaken for a
    // command, or the slice below silently pairs the wrong two lines.
    let run_commands: Vec<&str> = recipe
        .lines()
        .map(str::trim_start)
        .filter(|line| line.starts_with("$(GATE_RUSTFLAGS)") && line.contains("nextest run"))
        .collect();
    let [prewarm_command, sweep_command] = run_commands.as_slice() else {
        anyhow::bail!("expected prewarm then sweep, found {run_commands:?}");
    };
    for command in &run_commands {
        ensure!(
            command.contains("--test kani_mutation_evidence_tests"),
            "wrong test binary: {command:?}"
        );
        ensure!(
            command.contains("--run-ignored ignored-only"),
            "ignored gate was not selected: {command:?}"
        );
        ensure!(
            command.contains("$(GATE_RUSTFLAGS)"),
            "gate flags missing: {command:?}"
        );
    }
    ensure!(
        prewarm_command.contains("prewarm::prewarm_kani_mutation_target")
            && prewarm_command
                .contains("negative::compile_kani_tree_rejects_trees_that_do_not_compile")
            && sweep_command
                .contains("compile_guard::every_patched_tree_compiles_under_denied_warnings"),
        "the cold prewarm and its negative check must precede the filtered \
         patch sweep: {run_commands:?}"
    );
    ensure_the_prewarm_outranks_its_negative_check()
}

/// Require the prewarm to outrank the negative check that shares its invocation.
///
/// The `nested-cargo-builds` group serializes the two tests but does not order
/// them, so the priority is what puts the prewarm first. Without it a cold
/// target charges the dependency build to the negative check's allowance,
/// which is the cost the split exists to avoid.
fn ensure_the_prewarm_outranks_its_negative_check() -> Result<()> {
    let text = read_repo_file(Utf8Path::new(".config/nextest.toml"))?;
    let config: Value = text
        .parse()
        .context("nextest configuration should parse as TOML")?;
    let prewarm = override_priority(&config, "prewarm::prewarm_kani_mutation_target")?;
    let negative = override_priority(
        &config,
        "negative::compile_kani_tree_rejects_trees_that_do_not_compile",
    )?;
    ensure!(
        prewarm == 1 && prewarm > negative,
        "the prewarm must carry priority 1 so that it is scheduled before the \
         negative check, found {prewarm} and {negative}"
    );
    Ok(())
}

/// Read the `priority` of the override whose filter names `test_name`.
///
/// Both tests are overridden in the Kani profile, which is the one this gate
/// selects, so that profile is where their ordering is read. Nextest treats a
/// missing `priority` as zero, so an override that only inherits the default
/// reports `0`.
fn override_priority(config: &Value, test_name: &str) -> Result<i64> {
    let overrides = config
        .get("profile")
        .and_then(|profile| profile.get("kani"))
        .and_then(|profile| profile.get("overrides"))
        .and_then(Value::as_array)
        .context("nextest configuration should declare profile.kani.overrides")?;
    let entry = overrides
        .iter()
        .find(|entry| {
            entry
                .get("filter")
                .and_then(Value::as_str)
                .is_some_and(|filter| filter.contains(test_name))
        })
        .with_context(|| format!("nextest configuration should override {test_name}"))?;
    Ok(entry
        .get("priority")
        .and_then(Value::as_integer)
        .unwrap_or(0))
}
