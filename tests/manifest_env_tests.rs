//! Tests for injected environment access through the manifest `env()` helper.

use anyhow::{Context, Result, anyhow, ensure};
use netsuke::{
    ast::Recipe,
    manifest::{self, EnvReadError, EnvReader},
};
use rstest::rstest;
use std::sync::Arc;
use test_support::{
    EnLocalizer, en_localizer, fluent::normalize_fluent_isolates, manifest::manifest_yaml,
};

fn reader_yielding(result: Result<String, EnvReadError>) -> EnvReader {
    Arc::new(move |key| {
        if key == "PROFILE" {
            result.clone()
        } else {
            Err(EnvReadError::NotPresent)
        }
    })
}

#[test]
fn reader_yielding_rejects_unexpected_keys() {
    let reader = reader_yielding(Ok(String::from("value")));

    assert_eq!(reader("WRONG_KEY"), Err(EnvReadError::NotPresent));
}
fn rendered_command(value: Result<String, EnvReadError>) -> Result<String> {
    let yaml =
        manifest_yaml("targets:\n  - name: hello\n    command: \"echo {{ env('PROFILE') }}\"\n");
    let manifest = manifest::from_str_with_env(&yaml, &reader_yielding(value))?;
    let target = manifest
        .targets
        .first()
        .context("manifest should contain a target")?;
    let Recipe::Command { command } = &target.recipe else {
        return Err(anyhow!("expected command recipe, got {:?}", target.recipe));
    };
    command
        .as_single()
        .map(str::to_owned)
        .context("command should be a scalar")
}

#[rstest]
#[case::plain("world", "echo world")]
#[case::empty("", "echo ")]
#[case::special("spaced value $with #symbols", "echo spaced value $with #symbols")]
fn injected_values_render(#[case] value: &str, #[case] expected: &str) -> Result<()> {
    ensure!(rendered_command(Ok(value.to_owned()))? == expected);
    Ok(())
}

/// The helper must receive the name the template asked for.
#[rstest]
fn the_template_variable_name_reaches_the_reader() -> Result<()> {
    let seen: Arc<std::sync::Mutex<Vec<String>>> = Arc::new(std::sync::Mutex::new(Vec::new()));
    let recorder = Arc::clone(&seen);
    let reader: EnvReader = Arc::new(move |key: &str| {
        if let Ok(mut names) = recorder.lock() {
            names.push(key.to_owned());
        }
        Ok(String::from("value"))
    });
    let yaml = manifest_yaml(
        "targets:\n  - name: hello\n    command: \"echo {{ env('NETSUKE_SPECIFIC_NAME') }}\"\n",
    );
    manifest::from_str_with_env(&yaml, &reader)?;
    let names = seen.lock().map_err(|_| anyhow!("recorder poisoned"))?;
    ensure!(
        names.iter().any(|name| name == "NETSUKE_SPECIFIC_NAME"),
        "expected the template's variable name, got {names:?}"
    );
    Ok(())
}

#[rstest]
#[case::missing(EnvReadError::NotPresent, "is not set")]
#[case::non_utf8(EnvReadError::NotUnicode, "invalid utf-8")]
fn lookup_failures_are_diagnostic(
    #[case] failure: EnvReadError,
    #[case] expected: &str,
) -> Result<()> {
    let error = rendered_command(Err(failure)).expect_err("lookup should fail");
    ensure!(
        error
            .chain()
            .any(|cause| cause.to_string().to_lowercase().contains(expected)),
        "unexpected error: {error}"
    );
    Ok(())
}

fn localized_lookup_failure(failure: EnvReadError) -> Result<String> {
    let Err(error) = rendered_command(Err(failure)) else {
        return Err(anyhow!("environment lookup should fail"));
    };
    let message = error
        .chain()
        .map(ToString::to_string)
        .find(|message| message.contains("environment variable"))
        .context("lookup error should contain its localized environment diagnostic")?;
    Ok(normalize_fluent_isolates(&message))
}

#[rstest]
fn missing_lookup_diagnostic_snapshot(en_localizer: EnLocalizer) -> Result<()> {
    let _en = en_localizer;
    let message = localized_lookup_failure(EnvReadError::NotPresent)?;
    insta::assert_snapshot!(message, @"undefined value: A required environment variable is not set. (in <string>:1)");
    Ok(())
}

#[rstest]
fn non_unicode_lookup_diagnostic_snapshot(en_localizer: EnLocalizer) -> Result<()> {
    let _en = en_localizer;
    let message = localized_lookup_failure(EnvReadError::NotUnicode)?;
    insta::assert_snapshot!(message, @"invalid operation: An environment variable contains invalid UTF-8. (in <string>:1)");
    Ok(())
}
