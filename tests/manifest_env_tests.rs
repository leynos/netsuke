//! Tests for environment variable access via the Jinja `env()` helper.
//!
//! These drive the real registration path — `from_str_with_env` builds the
//! Jinja environment and registers `env()` exactly as `from_str` does — with an
//! injected reader rather than a mutated process. Nothing here is `#[serial]`:
//! the cases are independent and run concurrently.
//!
//! The non-UTF-8 case no longer needs `OsStringExt` surgery on a live variable,
//! so it is exercised on every platform rather than only Unix.

use anyhow::{Context, Result, anyhow, ensure};
use netsuke::{
    ast::Recipe,
    manifest::{self, EnvReader},
};
use rstest::rstest;
use std::env::VarError;
use std::ffi::OsString;
use std::sync::Arc;
use test_support::manifest::manifest_yaml;

/// A reader answering every lookup with `result`.
fn reader_yielding(result: Result<String, VarError>) -> EnvReader {
    Arc::new(move |_| result.clone())
}

fn command_for(yaml: &str, reader: &EnvReader) -> Result<String> {
    let manifest = manifest::from_str_with_env(yaml, reader)?;
    let first = manifest
        .targets
        .first()
        .context("manifest should contain at least one target")?;
    let Recipe::Command { command } = &first.recipe else {
        return Err(anyhow!("expected command recipe, got {:?}", first.recipe));
    };
    Ok(command.clone())
}

fn manifest_reading(key: &str) -> String {
    manifest_yaml(&format!(
        "targets:\n  - name: hello\n    command: \"echo {{{{ env('{key}') }}}}\"\n"
    ))
}

#[rstest]
#[case::plain("world", "echo world")]
#[case::empty("", "echo ")]
#[case::special("spaced value $with #symbols", "echo spaced value $with #symbols")]
fn env_var_renders_parameterized(
    #[case] env_value: &str,
    #[case] expected_command: &str,
) -> Result<()> {
    let reader = reader_yielding(Ok(env_value.to_owned()));
    let command = command_for(&manifest_reading("NETSUKE_ENV_TEST"), &reader)?;
    ensure!(
        command == expected_command,
        "expected command {expected_command}, got {command}"
    );
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
    command_for(&manifest_reading("NETSUKE_SPECIFIC_NAME"), &reader)?;
    let names = seen.lock().map_err(|_| anyhow!("recorder poisoned"))?;
    ensure!(
        names.iter().any(|name| name == "NETSUKE_SPECIFIC_NAME"),
        "expected the template's variable name, got {names:?}"
    );
    Ok(())
}

#[rstest]
#[case::missing(VarError::NotPresent, "is not set")]
#[case::non_utf8(VarError::NotUnicode(OsString::from("f\u{fffd}o")), "invalid utf-8")]
fn failures_surface_the_documented_message(
    #[case] var_error: VarError,
    #[case] expected_fragment: &str,
) -> Result<()> {
    let reader = reader_yielding(Err(var_error));
    match manifest::from_str_with_env(&manifest_reading("NETSUKE_ENV_FAILING"), &reader) {
        Ok(manifest) => Err(anyhow!(
            "expected the env lookup to fail, rendered manifest: {manifest:?}"
        )),
        Err(err) => {
            ensure!(
                err.chain()
                    .any(|e| e.to_string().to_lowercase().contains(expected_fragment)),
                "expected an error mentioning {expected_fragment:?}, got: {err}"
            );
            Ok(())
        }
    }
}
