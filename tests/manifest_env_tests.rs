//! Tests for injected environment access through the manifest `env()` helper.

use anyhow::{Context, Result, anyhow, ensure};
use netsuke::{
    ast::Recipe,
    manifest::{self, EnvReader},
};
use rstest::rstest;
use std::{env::VarError, ffi::OsString, sync::Arc};
use test_support::manifest::manifest_yaml;

fn reader_yielding(result: Result<String, VarError>) -> EnvReader {
    Arc::new(move |_| result.clone())
}

fn rendered_command(value: Result<String, VarError>) -> Result<String> {
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
    Ok(command.clone())
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
#[case::missing(VarError::NotPresent, "is not set")]
#[case::non_utf8(VarError::NotUnicode(OsString::from("bad")), "invalid utf-8")]
fn lookup_failures_are_diagnostic(#[case] failure: VarError, #[case] expected: &str) -> Result<()> {
    let error = rendered_command(Err(failure)).expect_err("lookup should fail");
    ensure!(
        error
            .chain()
            .any(|cause| cause.to_string().to_lowercase().contains(expected)),
        "unexpected error: {error}"
    );
    Ok(())
}
