//! Tests for injected environment access through the manifest `env()` helper.

use anyhow::{Context, Result, anyhow, ensure};
use netsuke::{
    ast::Recipe,
    manifest::{self, EnvAccessPolicy, EnvReadError, EnvReader},
};
use rstest::rstest;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
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
    let policy = EnvAccessPolicy::default();
    rendered_command_with_policy(value, &policy)
}

fn rendered_command_with_policy(
    value: Result<String, EnvReadError>,
    policy: &EnvAccessPolicy,
) -> Result<String> {
    let yaml =
        manifest_yaml("targets:\n  - name: hello\n    command: \"echo {{ env('PROFILE') }}\"\n");
    let manifest = manifest::from_str_with_env_and_policy(&yaml, &reader_yielding(value), policy)?;
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
fn allowlisted_lookup_resolves_successfully() -> Result<()> {
    let command = rendered_command_with_policy(
        Ok(String::from("permitted")),
        &EnvAccessPolicy::default().allow_var("PROFILE"),
    )?;
    ensure!(command == "echo permitted");
    Ok(())
}

#[rstest]
fn blocked_lookup_is_value_and_name_free() -> Result<()> {
    const VARIABLE_NAME: &str = "CREDENTIAL_LIKE_VARIABLE";
    const VARIABLE_VALUE: &str = "credential-like-value";
    let yaml = manifest_yaml(&format!(
        "targets:\n  - name: hello\n    command: \"echo {{{{ env('{VARIABLE_NAME}') }}}}\"\n"
    ));
    let reader_was_called = Arc::new(AtomicBool::new(false));
    let invocation_recorder = Arc::clone(&reader_was_called);
    let reader: EnvReader = Arc::new(move |_| {
        invocation_recorder.store(true, Ordering::Relaxed);
        Ok(String::from(VARIABLE_VALUE))
    });
    let error = manifest::from_str_with_env_and_policy(
        &yaml,
        &reader,
        &EnvAccessPolicy::default().block_var(VARIABLE_NAME),
    )
    .expect_err("a blocked environment variable must fail");
    let diagnostic = format!("{error:#}");
    ensure!(diagnostic.contains("Access to an environment variable is blocked."));
    ensure!(
        !reader_was_called.load(Ordering::Relaxed),
        "a blocked lookup must not call the reader"
    );
    ensure!(!diagnostic.contains(VARIABLE_NAME));
    ensure!(!diagnostic.contains(VARIABLE_VALUE));
    Ok(())
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

#[rstest]
fn blocked_lookup_diagnostic_snapshot(en_localizer: EnLocalizer) -> Result<()> {
    let _en = en_localizer;
    let error = rendered_command_with_policy(
        Ok(String::from("must-not-render")),
        &EnvAccessPolicy::default().block_var("PROFILE"),
    )
    .expect_err("blocked environment variable must fail");
    let message = error
        .chain()
        .map(ToString::to_string)
        .find(|message| message.contains("environment variable"))
        .context("blocked lookup should contain its localized environment diagnostic")?;
    let normalized_message = normalize_fluent_isolates(&message);
    insta::assert_snapshot!(normalized_message, @"invalid operation: Access to an environment variable is blocked. (in <string>:1)");
    Ok(())
}
