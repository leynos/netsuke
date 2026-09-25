//! `OBL-ENV-DEFAULT` at the template layer: what `default` accepts, and what
//! it rejects.
//!
//! These drive `env()` through a rendered manifest rather than through the
//! `env_var_with_default` seam, because argument *parsing* is `MiniJinja`'s
//! and belongs to the registration the manifest loader builds. The seam's own
//! partition is enumerated in `src/manifest/tests/env_function.rs`.

use super::reader_yielding;
use anyhow::{Context, Result, anyhow, ensure};
use netsuke::{
    ast::Recipe,
    manifest::{self, EnvAccessPolicy, EnvReadError, EnvReader},
};
use rstest::rstest;
use test_support::{fluent::normalize_fluent_isolates, manifest::manifest_yaml};

/// Render `env('PROFILE' <argument>)` and return the resulting command.
fn rendered_command_with_default(
    value: Result<String, EnvReadError>,
    argument: &str,
) -> Result<String> {
    let yaml = manifest_yaml(&format!(
        "targets:\n  - name: hello\n    command: \"echo {{{{ env('PROFILE'{argument}) }}}}\"\n"
    ));
    render_first_command(&yaml, &reader_yielding(value))
}

/// Render an arbitrary manifest template under the default policy.
fn rendered_command_with_argument(template: &str) -> Result<String> {
    let yaml = manifest_yaml(&format!(
        "targets:\n  - name: hello\n    command: \"{template}\"\n"
    ));
    render_first_command(&yaml, &reader_yielding(Ok(String::from("value"))))
}

/// Parse `yaml` with `reader` and return its sole target's command.
fn render_first_command(yaml: &str, reader: &EnvReader) -> Result<String> {
    let manifest =
        manifest::from_str_with_env_and_policy(yaml, reader, &EnvAccessPolicy::default())?;
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

/// Render `template` and return the error it is rejected with.
///
/// The rejection tests differ only in their template and expected text, so they
/// share one body rather than each repeating the render and the failure branch.
/// Returning the error lets a caller name what it expects in the chain, and
/// keeps the `let … else` shape this module's sibling uses — `clippy.toml`
/// exempts `expect` inside `#[test]` bodies, which a shared helper is not.
fn rejection_of(template: &str) -> Result<anyhow::Error> {
    let Err(error) = rendered_command_with_argument(template) else {
        return Err(anyhow!("the template must be rejected"));
    };
    Ok(error)
}

/// Render `template` and assert it is rejected with `expected` in the chain.
fn ensure_template_is_rejected(template: &str, expected: &str) -> Result<()> {
    let error = rejection_of(template)?;
    ensure!(
        error
            .chain()
            .any(|cause| cause.to_string().contains(expected)),
        "the diagnostic should contain {expected:?}, got {error:#}"
    );
    Ok(())
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
    let without = super::rendered_command(Err(EnvReadError::NotPresent))
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
    ensure_template_is_rejected(
        "echo {{ env('PROFILE', 'fallback') }}",
        "too many arguments",
    )
}

/// An unknown keyword argument is rejected, naming the key.
#[test]
fn an_unknown_keyword_argument_is_rejected() -> Result<()> {
    ensure_template_is_rejected("echo {{ env('PROFILE', defualt='x') }}", "defualt")
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
