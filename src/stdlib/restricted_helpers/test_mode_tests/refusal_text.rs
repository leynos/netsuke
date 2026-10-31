//! The typed refusal, its call shapes, and its rendered text.
//!
//! Split from `test_mode_tests.rs`, which exceeded the module line cap, along
//! the claim it serves: a refusal is only useful if a caller can recognise it
//! without matching diagnostic text, so these cases render against a bare
//! `MiniJinja` environment rather than loading a manifest, and the two text
//! cases then pin what a manifest author actually sees.

use std::error::Error as _;

use anyhow::{Context as _, Result, ensure};
use minijinja::{Environment, ErrorKind};
use rstest::rstest;
use test_support::{
    EnLocalizer, en_localizer, fluent::normalize_fluent_isolates, localizer::locale_localizer,
};

use super::super::{
    RestrictedSurface, SURFACE, TestModeRefusal, register_for_test, register_glob_refusal,
};
use super::{probe_config, probe_expression};

/// Build an environment holding only the test surface and its refusals.
fn refusal_environment() -> Environment<'static> {
    let mut env = Environment::new();
    let config = probe_config();
    let _state = register_for_test(&mut env, &config);
    register_glob_refusal(&mut env, RestrictedSurface::Test);
    env
}

/// Assert a rendered expression refuses with the typed source and operation.
fn assert_typed_refusal(env: &Environment<'_>, expression: &str, operation: &str) -> Result<()> {
    let error = env
        .render_str(expression, ())
        .expect_err(&format!("`{expression}` must refuse"));
    ensure!(
        error.kind() == ErrorKind::InvalidOperation,
        "`{expression}` reported {kind:?}, not an invalid operation",
        kind = error.kind()
    );
    let refusal = error
        .source()
        .and_then(|source| source.downcast_ref::<TestModeRefusal>())
        .with_context(|| format!("`{expression}` carried no typed refusal"))?;
    ensure!(
        refusal.operation() == operation,
        "`{expression}` refused as `{}`, not `{operation}`",
        refusal.operation()
    );
    Ok(())
}

/// Assert every refusing row carries a typed source naming the helper.
#[test]
fn refusals_carry_a_typed_source() -> Result<()> {
    let env = refusal_environment();
    let mut failures = Vec::new();
    for entry in SURFACE
        .iter()
        .filter(|entry| RestrictedSurface::Test.refuses(entry))
    {
        let Some(expression) = probe_expression(entry) else {
            failures.push(format!("{}: no probe expression", entry.name));
            continue;
        };
        if let Err(error) = assert_typed_refusal(&env, expression, entry.name) {
            failures.push(format!("{}: {error:#}", entry.name));
        }
    }
    ensure!(
        failures.is_empty(),
        "{} refusing rows lacked a typed source:\n{}",
        failures.len(),
        failures.join("\n")
    );
    Ok(())
}

/// Assert a refusal is reached from both positional and keyword call shapes.
#[test]
fn refusals_accept_any_call_shape() -> Result<()> {
    let env = refusal_environment();
    for expression in [
        "{{ fetch('https://example.invalid') }}",
        "{{ fetch(url='https://example.invalid') }}",
        "{{ 'probe' | hash('sha256') }}",
        "{{ 'probe' | hash() }}",
        "{{ 'probe' is file }}",
    ] {
        assert_typed_refusal(&env, expression, expr_operation(expression))?;
    }
    Ok(())
}

/// Return the helper name an expression in the call-shape table exercises.
fn expr_operation(expression: &str) -> &'static str {
    if expression.contains("fetch") {
        "fetch"
    } else if expression.contains("hash") {
        "hash"
    } else {
        "file"
    }
}

/// Pin the English refusal text for one helper.
#[rstest]
fn english_refusal_text_is_pinned(en_localizer: EnLocalizer) {
    let _localizer = en_localizer;
    let text = normalize_fluent_isolates(&TestModeRefusal::new("hash").to_string());
    insta::assert_snapshot!(
        text,
        @"[netsuke::jinja::test_mode::unavailable] hash is unavailable under `netsuke test`."
    );
}

/// Assert the refusal leads with its code under a non-English catalogue.
#[test]
fn refusal_text_carries_the_code_under_another_locale() {
    let _guards = locale_localizer("ja");
    let text = normalize_fluent_isolates(&TestModeRefusal::new("hash").to_string());
    assert!(
        text.starts_with("[netsuke::jinja::test_mode::unavailable] hash"),
        "Japanese refusal did not lead with the code and helper name: {text}"
    );
}
