//! Tests covering manifest macro parsing and registration.
use super::super::jinja_macros::{
    call_macro_value, parse_macro_name, register_macro, register_manifest_macros,
};
use super::super::{ManifestMap, ManifestValue};
use crate::ast::MacroDefinition;
use crate::localization::{self, keys};
use anyhow::{Context, Result as AnyResult, anyhow, ensure};
use minijinja::value::{Kwargs, Value};
use minijinja::{Environment, UndefinedBehavior};
use rstest::{fixture, rstest};
use std::sync::Arc;
use test_support::localizer_test_lock;

struct MacroRenderCase<'a> {
    signature: &'a str,
    body: &'a str,
    template: &'a str,
    expected: &'a str,
}

fn render_with(env: &Environment, template: &str) -> AnyResult<String> {
    Ok(env.render_str(template, ())?)
}

#[fixture]
fn strict_env() -> Environment<'static> {
    let mut env = Environment::new();
    env.set_undefined_behavior(UndefinedBehavior::Strict);
    env
}

type EnLocalizerFixture = (
    std::sync::MutexGuard<'static, ()>,
    crate::localization::LocalizerGuard,
);

#[fixture]
fn en_localizer() -> AnyResult<EnLocalizerFixture> {
    let lock = localizer_test_lock()
        .map_err(|e| anyhow::Error::msg(e.to_string()))
        .context("localizer test lock poisoned")?;
    let localizer = crate::cli_localization::build_localizer(Some("en-US"));
    let guard = localization::set_localizer_for_tests(Arc::from(localizer));
    Ok((lock, guard))
}

#[rstest]
#[case("greet(name)", "greet")]
#[case("shout(text='hi')", "shout")]
#[case("joiner(*items)", "joiner")]
#[case("format(name, caller=None)", "format")]
#[case("complex(value, /, *, flag=false, **kw)", "complex")]
fn parse_macro_name_extracts_identifier(
    #[case] signature: &str,
    #[case] expected: &str,
) -> AnyResult<()> {
    let name = parse_macro_name(signature)?;
    ensure!(name == expected, "expected {expected}, got {name}");
    Ok(())
}

#[rstest]
#[case("greet", keys::MANIFEST_MACRO_SIGNATURE_MISSING_PARAMS)]
#[case("(name)", keys::MANIFEST_MACRO_SIGNATURE_MISSING_IDENTIFIER)]
#[case("   ", keys::MANIFEST_MACRO_SIGNATURE_MISSING_IDENTIFIER)]
fn parse_macro_name_errors(
    en_localizer: AnyResult<EnLocalizerFixture>,
    #[case] signature: &str,
    #[case] key: &'static str,
) -> AnyResult<()> {
    let (_lock, _guard) = en_localizer?;
    let expected = localization::message(key).to_string();
    match parse_macro_name(signature) {
        Ok(name) => Err(anyhow!(
            "expected parse_macro_name to fail for {signature:?} but succeeded with {name}"
        )),
        Err(err) => {
            ensure!(
                err.to_string().contains(&expected),
                "expected error to contain {expected:?}, got {err:?}"
            );
            Ok(())
        }
    }
}

#[rstest]
#[case(MacroRenderCase {
    signature: "greet()",
    body: "Hello",
    template: "{{ greet() }}",
    expected: "Hello",
})]
#[case(MacroRenderCase {
    signature: "echo(text='hi')",
    body: "{{ text }}",
    template: "{{ echo() }}",
    expected: "hi",
})]
#[case(MacroRenderCase {
    signature: "joiner(items)",
    body: "{{ items | join(',') }}",
    template: "{{ joiner(['a', 'b', 'c']) }}",
    expected: "a,b,c",
})]
#[case(MacroRenderCase {
    signature: "show(name, excited=false)",
    body: "{{ name ~ ('!' if excited else '') }}",
    template: "{{ show('Netsuke', excited=true) }}",
    expected: "Netsuke!",
})]
#[case(MacroRenderCase {
    signature: "salute(name='friend')",
    body: "Hello {{ name }}",
    template: "{{ salute(name='Ada') }}",
    expected: "Hello Ada",
})]
#[case(MacroRenderCase {
    signature: "wrap(prefix, caller)",
    body: "{{ prefix }}{{ caller() }}",
    template: "{% call wrap('Hi ') %}World{% endcall %}",
    expected: "Hi World",
})]
fn register_macro_handles_arguments(
    #[case] case: MacroRenderCase,
    mut strict_env: Environment<'static>,
) -> AnyResult<()> {
    let macro_def = MacroDefinition {
        signature: case.signature.to_owned(),
        body: case.body.to_owned(),
    };
    register_macro(&mut strict_env, &macro_def, 0)?;
    let rendered = render_with(&strict_env, case.template)?;
    ensure!(
        rendered == case.expected,
        "expected {}, got {rendered}",
        case.expected
    );
    Ok(())
}

#[rstest]
fn call_macro_value_supports_kwargs(mut strict_env: Environment<'static>) -> AnyResult<()> {
    strict_env.add_template(
        "macro",
        "{% macro greet(name='friend') %}hi {{ name }}{% endmacro %}",
    )?;
    let template = strict_env
        .get_template("macro")
        .context("missing template")?;
    let state = template.eval_to_state(())?;
    let value = state
        .lookup("greet")
        .context("macro value missing")?
        .clone();
    let kwargs = Kwargs::from_iter([("name".to_owned(), Value::from("Ada"))]);
    let rendered = call_macro_value(&state, &value, &[], Some(kwargs))?;
    ensure!(
        rendered.to_string() == "hi Ada",
        "unexpected rendered output"
    );
    Ok(())
}

#[rstest]
fn register_macro_is_reusable(mut strict_env: Environment<'static>) -> AnyResult<()> {
    let macro_def = MacroDefinition {
        signature: "echo(text)".to_owned(),
        body: "{{ text }}".to_owned(),
    };
    register_macro(&mut strict_env, &macro_def, 0)?;

    let template = "{{ echo('first') }} {{ echo('second') }}";
    let rendered = render_with(&strict_env, template)?;
    ensure!(rendered.trim() == "first second");

    // Re-render to ensure the cached macro value remains valid.
    let rendered_again = render_with(&strict_env, template)?;
    ensure!(rendered_again.trim() == "first second");
    Ok(())
}

/// Helper to verify macro registration fails with expected error.
fn assert_macro_registration_fails(
    en_localizer: AnyResult<EnLocalizerFixture>,
    doc: &ManifestValue,
    env: &mut Environment<'static>,
    fail_message: &str,
) -> AnyResult<()> {
    let (_lock, _guard) = en_localizer?;
    match register_manifest_macros(doc, env) {
        Ok(()) => Err(anyhow!("{fail_message}")),
        Err(err) => {
            let expected = localization::message(keys::MANIFEST_MACRO_SEQUENCE_INVALID).to_string();
            ensure!(
                err.to_string().contains(&expected),
                "expected error to contain {expected:?}, got {err:?}"
            );
            Ok(())
        }
    }
}

#[rstest]
fn register_manifest_macros_validates_shape(
    mut strict_env: Environment<'static>,
    en_localizer: AnyResult<EnLocalizerFixture>,
) -> AnyResult<()> {
    let mut mapping = ManifestMap::new();
    mapping.insert(
        "macros".into(),
        ManifestValue::Array(vec![ManifestValue::from(42)]),
    );
    let doc = ManifestValue::Object(mapping);
    assert_macro_registration_fails(
        en_localizer,
        &doc,
        &mut strict_env,
        "expected error for non-mapping macro entry",
    )
}

/// Build a macro document from a mapping for testing.
fn build_macro_doc(macro_mapping: ManifestMap) -> ManifestValue {
    let macros = ManifestValue::Array(vec![ManifestValue::Object(macro_mapping)]);
    let mut doc = ManifestMap::new();
    doc.insert("macros".into(), macros);
    ManifestValue::Object(doc)
}

#[rstest]
#[case::non_string_body(
    {
        let mut m = ManifestMap::new();
        m.insert("signature".into(), ManifestValue::from("greet(name)"));
        m.insert("body".into(), ManifestValue::Number(serde_json::Number::from(42)));
        m
    },
    "register_manifest_macros should fail when macro body is not a string"
)]
#[case::missing_body(
    {
        let mut m = ManifestMap::new();
        m.insert("signature".into(), ManifestValue::from("greet(name)"));
        m
    },
    "expected missing macro body to trigger an error"
)]
fn register_manifest_macros_invalid_macro_entry(
    mut strict_env: Environment<'static>,
    en_localizer: AnyResult<EnLocalizerFixture>,
    #[case] macro_mapping: ManifestMap,
    #[case] fail_message: &str,
) -> AnyResult<()> {
    let doc = build_macro_doc(macro_mapping);
    assert_macro_registration_fails(en_localizer, &doc, &mut strict_env, fail_message)
}

#[test]
fn manifest_macros_with_non_string_keys_fail_to_parse() -> AnyResult<()> {
    let yaml = r#"
macros:
  - ? [not, string]
    : signature: "greet(name)"
      body: "Hello"
"#;
    match serde_saphyr::from_str::<ManifestValue>(yaml) {
        Ok(_) => Err(anyhow!("expected non-string keys to fail parsing")),
        Err(err) => {
            let msg = err.to_string();
            ensure!(
                msg.contains("expected string scalar")
                    || msg.contains("key")
                    || msg.contains("mapping"),
                "{msg}"
            );
            Ok(())
        }
    }
}

#[rstest]
fn register_manifest_macros_supports_multiple(
    mut strict_env: Environment<'static>,
) -> AnyResult<()> {
    let yaml = serde_saphyr::from_str::<ManifestValue>(concat!(
        "macros:\n",
        "  - signature: \"greet(name)\"\n",
        "    body: |\n",
        "      Hello {{ name }}\n",
        "  - signature: \"shout(text)\"\n",
        "    body: |\n",
        "      {{ text | upper }}\n",
    ))?;
    register_manifest_macros(&yaml, &mut strict_env)?;
    let rendered = render_with(&strict_env, "{{ shout(greet('netsuke')) }}")?;
    ensure!(rendered.trim() == "HELLO NETSUKE");
    Ok(())
}
