//! Safe invocation helpers for manifest-defined Jinja macros.

use super::call::call_macro_value;
use super::telemetry;
use crate::localization::{self, keys};
use minijinja::{
    AutoEscape, Captured, Environment, Error, ErrorKind, State,
    value::{Kwargs, Rest, Value},
};

#[derive(Clone, Copy)]
struct MacroReference<'a> {
    template_name: &'a str,
    macro_name: &'a str,
}

/// Build the global-function fallback used by compiled Jinja expressions.
///
/// Template rendering imports manifest macros into the active template state,
/// which is the path that supports Jinja call blocks. Compiled expressions do
/// not support imports, so this fallback creates a short-lived captured state
/// for each expression call instead of extending its lifetime unsafely.
///
/// Evaluation stays in [`invoke_macro`]; the callback only composes it with the
/// instrumentation boundary in [`telemetry`], so the query itself carries no
/// timing or metric concerns.
pub(super) fn make_macro_fn(
    template_name: String,
    macro_name: String,
) -> impl Fn(&State, Rest<Value>, Kwargs) -> Result<Value, Error> {
    telemetry::describe_macro_metrics();
    move |state, Rest(args), macro_kwargs| {
        let reference = MacroReference {
            template_name: &template_name,
            macro_name: &macro_name,
        };
        telemetry::instrument_macro_invocation(|| {
            invoke_macro(state, args.as_slice(), &macro_kwargs, reference)
        })
    }
}

fn invoke_macro(
    state: &State,
    args: &[Value],
    macro_kwargs: &Kwargs,
    reference: MacroReference<'_>,
) -> Result<Value, Error> {
    let (captured, macro_value) =
        capture_macro(state.env(), reference.template_name, reference.macro_name)?;
    let maybe_kwargs = collect_kwargs(macro_kwargs)?;
    let rendered_value = call_macro_value(captured.state(), &macro_value, args, maybe_kwargs)?;
    let rendered: String = rendered_value.into();
    Ok(if matches!(state.auto_escape(), AutoEscape::None) {
        Value::from(rendered)
    } else {
        Value::from_safe_string(rendered)
    })
}

/// Confirm that a compiled template exports the requested macro.
pub(super) fn validate_macro(
    env: &Environment,
    template_name: &str,
    macro_name: &str,
) -> anyhow::Result<()> {
    capture_macro(env, template_name, macro_name)?;
    Ok(())
}

fn capture_macro<'source>(
    env: &'source Environment<'source>,
    template_name: &str,
    macro_name: &str,
) -> Result<(Captured<'source>, Value), Error> {
    let template = env.get_template(template_name).map_err(|error| {
        Error::new(
            error.kind(),
            localization::message(keys::MANIFEST_MACRO_TEMPLATE_LOAD_FAILED)
                .with_arg("template", template_name)
                .to_string(),
        )
        .with_source(error)
    })?;
    let captured = template.render_captured(()).map_err(|error| {
        Error::new(
            error.kind(),
            localization::message(keys::MANIFEST_MACRO_INIT_FAILED)
                .with_arg("macro", macro_name)
                .to_string(),
        )
        .with_source(error)
    })?;
    let value = captured.state().lookup(macro_name).ok_or_else(|| {
        Error::new(
            ErrorKind::InvalidOperation,
            localization::message(keys::MANIFEST_MACRO_MISSING)
                .with_arg("name", macro_name)
                .to_string(),
        )
    })?;
    Ok((captured, value))
}

fn collect_kwargs(macro_kwargs: &Kwargs) -> Result<Option<Kwargs>, Error> {
    let mut entries = Vec::new();
    for key in macro_kwargs.args() {
        entries.push((key.to_owned(), macro_kwargs.peek::<Value>(key)?));
    }
    Ok((!entries.is_empty()).then(|| entries.into_iter().collect()))
}

#[cfg(test)]
mod tests {
    //! Snapshots for localized macro-invocation failures.

    use super::{make_macro_fn, validate_macro};
    use minijinja::{Environment, ErrorKind, UndefinedBehavior};
    use rstest::rstest;
    use test_support::{EnLocalizer, en_localizer, fluent::normalize_fluent_isolates};

    #[rstest]
    fn missing_template_diagnostic_snapshot(en_localizer: EnLocalizer) {
        let _en = en_localizer;
        let env = Environment::new();
        let error = validate_macro(&env, "missing-template", "missing_macro")
            .expect_err("missing template should fail validation");

        insta::assert_snapshot!(error.to_string(), @"template not found: Failed to load macro template.");
    }

    #[rstest]
    fn initialization_diagnostic_snapshot(en_localizer: EnLocalizer) {
        let _en = en_localizer;
        let mut env = Environment::new();
        env.set_undefined_behavior(UndefinedBehavior::Strict);
        env.add_template("invalid-template", "{{ missing }}")
            .expect("invalid fixture template should compile");
        let error = validate_macro(&env, "invalid-template", "missing_macro")
            .expect_err("template initialization should fail validation");

        insta::assert_snapshot!(error.to_string(), @"undefined value: Failed to initialize macro environment.");
    }

    #[rstest]
    fn missing_macro_diagnostic_snapshot(en_localizer: EnLocalizer) {
        let _en = en_localizer;
        let mut env = Environment::new();
        env.add_template(
            "macro-template",
            "{% macro present() %}present{% endmacro %}",
        )
        .expect("macro fixture template should compile");
        let error = validate_macro(&env, "macro-template", "missing_macro")
            .expect_err("missing macro should fail validation");

        insta::assert_snapshot!(
            normalize_fluent_isolates(&error.to_string()),
            @"invalid operation: Macro missing_macro is missing."
        );
    }

    #[test]
    fn compiled_expression_invokes_macro_fallback() {
        let mut env = Environment::new();
        env.add_template(
            "macro-template",
            "{% macro greet(name) %}Hello {{ name }}{% endmacro %}",
        )
        .expect("macro fixture template should compile");
        env.add_function(
            "greet",
            make_macro_fn("macro-template".to_owned(), "greet".to_owned()),
        );

        let expression = env
            .compile_expression("greet('Ada')")
            .expect("macro expression should compile");
        let rendered = expression
            .eval(())
            .expect("compiled expression should invoke the macro fallback");

        assert_eq!(rendered.to_string(), "Hello Ada");
    }

    /// Keyword arguments must survive [`collect_kwargs`] and override the
    /// macro's default. Caller blocks are deliberately not covered here:
    /// compiled expressions support neither imports nor caller-block context,
    /// so that path is exercised through `render_template` in
    /// `crate::manifest::tests::macros::register_macro_handles_arguments`.
    #[test]
    fn compiled_expression_passes_keyword_arguments_over_defaults() {
        let mut env = Environment::new();
        env.add_template(
            "macro-template",
            "{% macro greet(name='World') %}Hello {{ name }}{% endmacro %}",
        )
        .expect("macro fixture template should compile");
        env.add_function(
            "greet",
            make_macro_fn("macro-template".to_owned(), "greet".to_owned()),
        );

        let expression = env
            .compile_expression("greet(name='Ada')")
            .expect("macro expression should compile");
        let rendered = expression
            .eval(())
            .expect("compiled expression should forward keyword arguments")
            .to_string();

        assert_eq!(
            rendered, "Hello Ada",
            "keyword argument should override the default"
        );
    }

    #[rstest]
    fn compiled_expression_reports_missing_fallback_template(en_localizer: EnLocalizer) {
        let _en = en_localizer;
        let mut env = Environment::new();
        env.add_function(
            "missing_macro",
            make_macro_fn("missing-template".to_owned(), "missing_macro".to_owned()),
        );

        let expression = env
            .compile_expression("missing_macro()")
            .expect("macro expression should compile");
        let error = expression
            .eval(())
            .expect_err("fallback invocation should reject a missing template");

        assert_eq!(error.kind(), ErrorKind::TemplateNotFound);
        assert_eq!(
            normalize_fluent_isolates(&error.to_string()),
            "template not found: Failed to load macro template. (in <expression>:1)"
        );
    }

    proptest::proptest! {
        /// Generated argument values must reach the macro output unchanged:
        /// the positional argument binds the first parameter and the keyword
        /// argument overrides the second parameter's default. Values are
        /// identifier-like ASCII so the Jinja source built from them cannot
        /// contain quoting or escape sequences.
        #[test]
        fn compiled_expression_forwards_generated_arguments(
            positional in "[a-z][a-z0-9_]{0,11}",
            keyword in "[a-z][a-z0-9_]{0,11}",
        ) {
            let mut env = Environment::new();
            env.add_template(
                "macro-template",
                "{% macro pair(first='left', second='right') %}{{ first }}:{{ second }}{% endmacro %}",
            )
            .expect("macro fixture template should compile");
            env.add_function(
                "pair",
                make_macro_fn("macro-template".to_owned(), "pair".to_owned()),
            );

            let source = format!("pair('{positional}', second='{keyword}')");
            let expression = env
                .compile_expression(&source)
                .expect("macro expression should compile");
            let rendered = expression
                .eval(())
                .expect("compiled expression should forward generated arguments")
                .to_string();

            proptest::prop_assert_eq!(rendered, format!("{positional}:{keyword}"));
        }
    }
}
