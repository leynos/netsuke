//! Startup-boundary tests for trusted configured shell definitions.
//!
//! The authority tests prove which sources may declare `shells`. These tests
//! prove what startup does with the decision: a rejection stops the process
//! before a manifest is compiled, and a later `json = true` still selects JSON
//! diagnostics. The bounded-diagnostic tests live in [`diagnostics`], and the
//! acceptance tests that a trusted definition does not block a build live in
//! [`success`].

mod diagnostics;
mod success;
mod support;

use anyhow::{Context, Result, ensure};
use rstest::rstest;
use support::{
    OPERATOR_CONFIG, RELATIVE_EXECUTABLE, SENTINEL, Startup, definition_body,
    ensure_rejected_before_compilation, generate_with_operator_config, json_failure, normalised,
    stderr_of,
};

#[test]
fn text_diagnostic_format_is_stable() -> Result<()> {
    let startup = Startup::new()?;

    let output = generate_with_operator_config(&startup, RELATIVE_EXECUTABLE)?;

    ensure!(
        !output.status.success(),
        "startup should reject the definition"
    );
    let rendered = normalised(&stderr_of(&output), &startup)?;
    insta::assert_snapshot!("shells_text_diagnostic", rendered);
    Ok(())
}

#[test]
fn json_diagnostic_format_is_stable() -> Result<()> {
    let startup = Startup::new()?;

    let output =
        generate_with_operator_config(&startup, &format!("json = true\n{RELATIVE_EXECUTABLE}"))?;

    json_failure(&output)?;
    let rendered = normalised(&stderr_of(&output), &startup)?;
    insta::assert_snapshot!("shells_json_diagnostic", rendered);
    Ok(())
}

#[test]
fn project_definition_fails_before_manifest_compilation() -> Result<()> {
    let startup = Startup::new()?;
    startup.write(".netsuke.toml", &definition_body("dash"))?;

    let output = startup.generate()?;

    ensure_rejected_before_compilation(&startup, &output, "shells")?;
    ensure!(
        stderr_of(&output).contains("operator"),
        "failure should state the operator-authority rule, got: {}",
        stderr_of(&output)
    );
    Ok(())
}

#[test]
fn explicit_selector_does_not_grant_authority() -> Result<()> {
    let startup = Startup::new()?;
    let untrusted = startup.write("untrusted.toml", &definition_body("custom"))?;
    let selector = untrusted.to_string_lossy().into_owned();

    let output = startup
        .command(&["--config", &selector, "generate", "--output", SENTINEL])
        .output()
        .context("run netsuke with an explicit selector")?;

    ensure_rejected_before_compilation(&startup, &output, "shells")
}

#[test]
fn environment_selector_does_not_grant_authority() -> Result<()> {
    let startup = Startup::new()?;
    let untrusted = startup.write("untrusted.toml", &definition_body("custom"))?;

    let output = startup
        .command(&["generate", "--output", SENTINEL])
        .env("NETSUKE_CONFIG", &untrusted)
        .output()
        .context("run netsuke with an environment selector")?;

    ensure_rejected_before_compilation(&startup, &output, "shells")
}

#[test]
fn inherited_project_definition_is_rejected() -> Result<()> {
    let startup = Startup::new()?;
    startup.write("base.toml", &definition_body("dash"))?;
    startup.write(".netsuke.toml", "extends = \"base.toml\"\n")?;

    let output = startup.generate()?;

    ensure_rejected_before_compilation(&startup, &output, "shells")
}

#[test]
fn malformed_trusted_definition_names_its_field() -> Result<()> {
    let startup = Startup::new()?;

    let output = generate_with_operator_config(&startup, RELATIVE_EXECUTABLE)?;

    ensure_rejected_before_compilation(&startup, &output, "shells[0].executable")?;
    ensure!(
        stderr_of(&output).contains("absolute"),
        "failure should explain the absolute-path rule, got: {}",
        stderr_of(&output)
    );
    Ok(())
}

#[test]
fn duplicate_trusted_definitions_are_rejected() -> Result<()> {
    let startup = Startup::new()?;
    startup.write(
        OPERATOR_CONFIG,
        &format!("{}{}", definition_body("dash"), definition_body("dash")),
    )?;

    let output = startup.generate()?;

    ensure_rejected_before_compilation(&startup, &output, "shells")?;
    let stderr = stderr_of(&output);
    ensure!(
        stderr.contains("shells[1].name") && stderr.contains("shells[0].name"),
        "failure should name both duplicate fields, got: {stderr}"
    );
    ensure!(
        stderr.contains("already declares"),
        "failure should explain the collision, got: {stderr}"
    );
    Ok(())
}

#[test]
fn later_json_preference_is_honoured_after_rejection() -> Result<()> {
    let startup = Startup::new()?;
    startup.write("base.toml", &definition_body("dash"))?;
    startup.write(".netsuke.toml", "extends = \"base.toml\"\njson = true\n")?;

    let output = startup.generate()?;

    json_failure(&output)?;
    ensure!(
        stderr_of(&output).contains("shells"),
        "JSON failure should name the rejected key, got: {}",
        stderr_of(&output)
    );
    ensure!(
        !startup.sentinel_exists(),
        "startup should stop before manifest compilation"
    );
    Ok(())
}

#[test]
fn malformed_operator_definition_honours_json_preference() -> Result<()> {
    let startup = Startup::new()?;

    let output =
        generate_with_operator_config(&startup, &format!("json = true\n{RELATIVE_EXECUTABLE}"))?;

    json_failure(&output)?;
    ensure!(
        stderr_of(&output).contains("shells[0].executable"),
        "JSON failure should name the rejected field, got: {}",
        stderr_of(&output)
    );
    Ok(())
}

#[rstest]
#[case::flat("NETSUKE_SHELLS")]
#[case::nested("NETSUKE_SHELLS__0__NAME")]
#[case::nested_arguments("NETSUKE_SHELLS__0__ARGS")]
#[case::mixed_case("netsuke_Shells")]
fn environment_aliases_are_denied(#[case] variable: &str) -> Result<()> {
    let startup = Startup::new()?;

    let output = startup
        .command(&["generate", "--output", SENTINEL])
        .env(variable, "dash")
        .output()
        .with_context(|| format!("run netsuke with {variable} set"))?;

    ensure!(!output.status.success(), "startup should reject {variable}");
    let stderr = stderr_of(&output);
    ensure!(
        stderr.contains("configuration file"),
        "{variable} should be rejected as file-only, got: {stderr}"
    );
    ensure!(
        !startup.sentinel_exists(),
        "{variable} should stop startup before manifest compilation"
    );
    Ok(())
}
