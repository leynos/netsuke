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

/// Assert that a denied lookup cannot reach or disclose an injected reader.
///
/// `argument` is appended verbatim inside the `env(...)` call, so `""`
/// exercises the bare form and `", default='fallback'"` exercises the form
/// that must not become an access-policy bypass. The fallback text is checked
/// for leakage here for the same reason the value is: the diagnostic is
/// user-visible, so neither may appear in it.
fn denied_lookup_is_value_and_name_free(
    policy: &EnvAccessPolicy,
    variable_name: &str,
    variable_value: &str,
    argument: &str,
) -> Result<()> {
    let yaml = manifest_yaml(&format!(
        "targets:\n  - name: hello\n    command: \"echo {{{{ env('{variable_name}'{argument}) }}}}\"\n"
    ));
    let reader_was_called = Arc::new(AtomicBool::new(false));
    let invocation_recorder = Arc::clone(&reader_was_called);
    let reader_value = variable_value.to_owned();
    let reader: EnvReader = Arc::new(move |_| {
        invocation_recorder.store(true, Ordering::Relaxed);
        Ok(reader_value.clone())
    });
    let Err(error) = manifest::from_str_with_env_and_policy(&yaml, &reader, policy) else {
        return Err(anyhow!("a denied environment variable must fail"));
    };
    let diagnostic = format!("{error:#}");
    ensure!(diagnostic.contains("Access to an environment variable is blocked."));
    ensure!(
        !reader_was_called.load(Ordering::Relaxed),
        "a denied lookup must not call the reader"
    );
    ensure!(!diagnostic.contains(variable_name));
    ensure!(!diagnostic.contains(variable_value));
    ensure!(
        !diagnostic.contains("fallback"),
        "the default must not appear in the diagnostic: {diagnostic}"
    );
    Ok(())
}

#[rstest]
fn blocked_lookup_is_value_and_name_free() -> Result<()> {
    denied_lookup_is_value_and_name_free(
        &EnvAccessPolicy::default().block_var("CREDENTIAL_LIKE_VARIABLE"),
        "CREDENTIAL_LIKE_VARIABLE",
        "credential-like-value",
        "",
    )
}

/// A `default` must not become an access-policy bypass.
///
/// `env('BLOCKED', default='x')` fails for the same reason `env('BLOCKED')`
/// does. The policy is evaluated before the reader, so the block cannot be
/// downgraded into a successful read by supplying a fallback — which is what a
/// naive "resolve, then substitute on absence" implementation would do.
#[test]
fn a_blocked_lookup_still_fails_when_a_default_is_supplied() -> Result<()> {
    denied_lookup_is_value_and_name_free(
        &EnvAccessPolicy::default().block_var("CREDENTIAL_LIKE_VARIABLE"),
        "CREDENTIAL_LIKE_VARIABLE",
        "credential-like-value",
        ", default='fallback'",
    )
}

#[rstest]
fn unlisted_allowlist_lookup_is_value_and_name_free() -> Result<()> {
    denied_lookup_is_value_and_name_free(
        &EnvAccessPolicy::default().allow_var("ANOTHER_VARIABLE"),
        "UNLISTED_CREDENTIAL_LIKE_VARIABLE",
        "unlisted-credential-like-value",
        "",
    )
}

#[rstest]
#[case::plain("world", "echo world")]
#[case::empty("", "echo ")]
#[case::special("spaced value $with #symbols", "echo spaced value $with #symbols")]
fn injected_values_render(#[case] value: &str, #[case] expected: &str) -> Result<()> {
    ensure!(rendered_command(Ok(value.to_owned()))? == expected);
    Ok(())
}

/// Render `env('PROFILE' …)` with `argument` appended, and return the command.
fn rendered_command_with_default(
    value: Result<String, EnvReadError>,
    argument: &str,
) -> Result<String> {
    let yaml = manifest_yaml(&format!(
        "targets:\n  - name: hello\n    command: \"echo {{{{ env('PROFILE'{argument}) }}}}\"\n"
    ));
    let manifest = manifest::from_str_with_env_and_policy(
        &yaml,
        &reader_yielding(value),
        &EnvAccessPolicy::default(),
    )?;
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

/// Render an arbitrary manifest template under the default policy.
fn rendered_command_with_argument(template: &str) -> Result<String> {
    let yaml = manifest_yaml(&format!(
        "targets:\n  - name: hello\n    command: \"{template}\"\n"
    ));
    let manifest = manifest::from_str_with_env_and_policy(
        &yaml,
        &reader_yielding(Ok(String::from("value"))),
        &EnvAccessPolicy::default(),
    )?;
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

/// `OBL-ENV-DEFAULT` at the template layer: what `default` accepts.
///
/// The reader is genuinely consulted in every row, so a row proving the default
/// was returned also proves the lookup happened; an implementation that ignored
/// the reader and returned `default` unconditionally would fail the
/// `present_ignores_default` row.
#[rstest]
#[case::present_ignores_default(
    Ok(String::from("present")),
    ", default='fallback'",
    "echo present"
)]
#[case::absent_uses_default(Err(EnvReadError::NotPresent), ", default='fallback'", "echo fallback")]
fn template_default_substitutes_for_absence(
    #[case] value: Result<String, EnvReadError>,
    #[case] argument: &str,
    #[case] expected: &str,
) -> Result<()> {
    ensure!(rendered_command_with_default(value, argument)? == expected);
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
        "unexpected error: {error:#}"
    );
    Ok(())
}

/// A non-UTF-8 value fails even when a valid `default` is supplied.
///
/// A present-but-undecodable value is a configuration fault, not an absence, so
/// the default must not paper over it. This is the one case where the plan's
/// own rule and a naive "read then fall back" implementation disagree.
#[test]
fn non_utf8_value_is_not_replaced_by_the_default() -> Result<()> {
    let error =
        rendered_command_with_default(Err(EnvReadError::NotUnicode), ", default='fallback'")
            .expect_err("a non-UTF-8 value must fail even with a default");
    ensure!(
        error
            .chain()
            .any(|cause| cause.to_string().to_lowercase().contains("invalid utf-8")),
        "unexpected error: {error:#}"
    );
    Ok(())
}

/// `default=none` is the absence of a default, not a default of the text "none".
///
/// The *rendered diagnostic* is compared, not just the error kind: constraint 1
/// freezes this wording, so an implementation that routed `none` down the
/// non-string branch would fail here even if it also failed the parse.
#[test]
fn explicit_none_default_is_equivalent_to_omitting_it() -> Result<()> {
    let with_none = rendered_command_with_default(Err(EnvReadError::NotPresent), ", default=none")
        .expect_err("an absent variable with default=none must still fail");
    let without = rendered_command(Err(EnvReadError::NotPresent))
        .expect_err("an absent variable with no default must fail");
    let with_none_lookup = environment_diagnostic(&with_none)
        .context("default=none should produce the missing-variable diagnostic")?;
    let without_lookup = environment_diagnostic(&without)
        .context("an omitted default should produce the missing-variable diagnostic")?;
    ensure!(
        with_none_lookup == without_lookup,
        "default=none should fail with the unchanged message an omitted default gives, got \
         {with_none_lookup:?} and {without_lookup:?}"
    );
    Ok(())
}

/// The localized environment message inside a manifest diagnostic, normalized.
fn environment_diagnostic(error: &anyhow::Error) -> Option<String> {
    let message = error
        .chain()
        .map(ToString::to_string)
        .find(|message| message.contains("environment variable"))?;
    Some(normalize_fluent_isolates(&message))
}

/// `default` is a keyword argument, so a bare second positional argument fails.
///
/// `MiniJinja` reports a positional overflow as a `TooManyArguments` with **no
/// detail**, naming neither the function nor the keyword it expected; that is
/// what makes this a rejection test rather than a guidance test. The template
/// author still gets a template location from the outer error.
#[test]
fn a_positional_second_argument_is_rejected() -> Result<()> {
    let error = rendered_command_with_argument("echo {{ env('PROFILE', 'fallback') }}")
        .expect_err("a positional second argument must be rejected");
    ensure!(
        error
            .chain()
            .any(|cause| cause.to_string().contains("too many arguments")),
        "unexpected error: {error:#}"
    );
    Ok(())
}

/// An unknown keyword argument is rejected, naming the key.
#[test]
fn an_unknown_keyword_argument_is_rejected() -> Result<()> {
    let error = rendered_command_with_argument("echo {{ env('PROFILE', defualt='x') }}")
        .expect_err("an unknown keyword argument must be rejected");
    ensure!(
        error
            .chain()
            .any(|cause| cause.to_string().contains("defualt")),
        "the diagnostic should name the offending keyword, got {error:#}"
    );
    Ok(())
}

/// `default` must be a string.
///
/// `Kwargs::get::<Option<String>>` would silently stringify every one of these
/// — `1` to `"1"`, `true` to the Python-shaped `"True"`, a sequence to a JSON
/// fragment — and paste the result into a shell recipe. `Option<Value>` plus an
/// explicit `as_str` check is what makes the coercion impossible; RFC 0006 §6.6
/// forbids it.
#[rstest]
#[case::number("1")]
#[case::boolean("true")]
#[case::sequence("['a','b']")]
#[case::mapping("{'a': 1}")]
fn a_non_string_default_is_rejected(#[case] literal: &str) -> Result<()> {
    let error = rendered_command_with_default(
        Err(EnvReadError::NotPresent),
        &format!(", default={literal}"),
    )
    .expect_err("a non-string default must be rejected");
    ensure!(
        error
            .chain()
            .any(|cause| cause.to_string().contains("must be a string")),
        "unexpected error: {error:#}"
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
