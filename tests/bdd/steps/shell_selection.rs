//! Bind the shell-selection vocabulary outline to its behavioural steps.

use crate::bdd::fixtures::TestWorld;
use anyhow::{Result, ensure};
use netsuke::shell_selection::{ShellName, ShellSelection};
use rstest_bdd_macros::{given, then, when};
use serde::Deserialize;

/// Decode a shell selector in a mapping that defaults when the key is absent.
#[derive(Deserialize)]
struct Probe {
    /// Hold the decoded selector from the behavioural document.
    #[serde(default)]
    shell: ShellSelection,
}

/// Verify that a BDD scenario produced its expected successful selection.
fn verify_selection(world: &TestWorld, expected: ShellSelection) -> Result<()> {
    let outcome = world
        .shell_selection_outcome
        .get()
        .ok_or_else(|| anyhow::anyhow!("shell selector has not been decoded"))?;
    ensure!(
        outcome == Ok(expected),
        "expected shell selection, got {outcome:?}"
    );
    Ok(())
}

/// Retain the outline cell verbatim so YAML quotes reach the parser.
#[expect(
    clippy::unnecessary_wraps,
    reason = "rstest-bdd macro generates a Result wrapper for step functions"
)]
#[given("the shell selector YAML {yaml}")]
fn shell_selector_yaml(world: &TestWorld, yaml: String) -> Result<()> {
    world.shell_selector_yaml.set(yaml);
    Ok(())
}

/// Decode through the same YAML-value and typed-value stages as the front end.
#[when("the shell selector is decoded")]
fn decode_shell_selector(world: &TestWorld) -> Result<()> {
    let yaml = world
        .shell_selector_yaml
        .get()
        .ok_or_else(|| anyhow::anyhow!("shell selector YAML has not been supplied"))?;
    let document = format!("shell: {yaml}\n");
    let outcome = serde_saphyr::from_str::<serde_json::Value>(&document)
        .map_err(|error| error.to_string())
        .and_then(|value| {
            serde_json::from_value::<Probe>(value)
                .map(|probe| probe.shell)
                .map_err(|error| error.to_string())
        });
    world.shell_selection_outcome.set(outcome);
    Ok(())
}

/// Verify that the Boolean selector chose direct execution.
#[then("the shell selection is direct execution")]
fn shell_selection_is_direct(world: &TestWorld) -> Result<()> {
    verify_selection(world, ShellSelection::Direct)
}

/// Verify that the Boolean selector chose the host-dependent default.
#[then("the shell selection is the platform default")]
fn shell_selection_is_platform_default(world: &TestWorld) -> Result<()> {
    verify_selection(world, ShellSelection::PlatformDefault)
}

/// Verify that the decoded selector retained the named shell.
#[then("the shell selection is the named shell {name:string}")]
fn shell_selection_is_named(world: &TestWorld, name: String) -> Result<()> {
    verify_selection(world, ShellSelection::Named(ShellName::parse(&name)?))
}

/// Verify that an invalid selector is rejected by the decoder.
#[then("decoding the shell selector fails")]
fn shell_selection_is_rejected(world: &TestWorld) -> Result<()> {
    let outcome = world
        .shell_selection_outcome
        .get()
        .ok_or_else(|| anyhow::anyhow!("shell selector has not been decoded"))?;
    ensure!(outcome.is_err(), "expected rejection, got {outcome:?}");
    Ok(())
}
