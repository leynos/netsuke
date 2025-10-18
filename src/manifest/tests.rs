//! Tests for manifest parsing and macro registration helpers.

use super::jinja_macros::{
    call_macro_value, parse_macro_name, register_macro, register_manifest_macros,
};
use super::*;
use crate::ast::MacroDefinition;
use anyhow::Result as AnyResult;
use minijinja::{
    Environment,
    value::{Kwargs, Value},
};
use rstest::{fixture, rstest};

fn render_with(env: &Environment, template: &str) -> AnyResult<String> {
    Ok(env.render_str(template, ())?)
}

#[fixture]
fn strict_env() -> Environment<'static> {
    let mut env = Environment::new();
    env.set_undefined_behavior(UndefinedBehavior::Strict);
    env
}

#[rstest]
#[case("greet(name)", "greet")]
#[case("shout(text='hi')", "shout")]
#[case("joiner(*items)", "joiner")]
#[case("format(name, caller=None)", "format")]
#[case("complex(value, /, *, flag=false, **kw)", "complex")]
fn parse_macro_name_extracts_identifier(#[case] signature: &str, #[case] expected: &str) {
    let name = parse_macro_name(signature).expect("parse name");
    assert_eq!(name, expected);
}

#[rstest]
#[case("greet", "include parameter list")]
#[case("(name)", "missing an identifier")]
#[case("   ", "missing an identifier")]
fn parse_macro_name_errors(#[case] signature: &str, #[case] message: &str) {
    let err = parse_macro_name(signature).expect_err("should fail");
    assert!(err.to_string().contains(message), "{err}");
}

#[rstest]
#[case("greet()", "Hello", "{{ greet() }}", "Hello")]
#[case("echo(text='hi')", "{{ text }}", "{{ echo() }}", "hi")]
#[case(
    "joiner(items)",
    "{{ items | join(',') }}",
    "{{ joiner(['a', 'b', 'c']) }}",
    "a,b,c"
)]
#[case(
    "show(name, excited=false)",
    "{{ name ~ ('!' if excited else '') }}",
    "{{ show('Netsuke', excited=true) }}",
    "Netsuke!"
)]
#[case(
    "salute(name='friend')",
    "Hello {{ name }}",
    "{{ salute(name='Ada') }}",
    "Hello Ada"
)]
#[case(
    "wrap(prefix, caller)",
    "{{ prefix }}{{ caller() }}",
    "{% call wrap('Hi ') %}World{% endcall %}",
    "Hi World"
)]
fn register_macro_handles_arguments(
    #[case] signature: &str,
    #[case] body: &str,
    #[case] template: &str,
    #[case] expected: &str,
    mut strict_env: Environment,
) {
    let macro_def = MacroDefinition {
        signature: signature.to_string(),
        body: body.to_string(),
    };
    register_macro(&mut strict_env, &macro_def, 0).expect("register");
    let rendered = render_with(&strict_env, template).expect("render");
    assert_eq!(rendered, expected);
}

#[rstest]
fn call_macro_value_supports_kwargs(mut strict_env: Environment) {
    strict_env
        .add_template(
            "macro",
            "{% macro greet(name='friend') %}hi {{ name }}{% endmacro %}",
        )
        .expect("template");
    let template = strict_env.get_template("macro").expect("template");
    let state = template.eval_to_state(()).expect("state");
    let value = state.lookup("greet").expect("macro").clone();
    let kwargs = Kwargs::from_iter([(String::from("name"), Value::from("Ada"))]);
    let rendered = call_macro_value(&state, &value, &[], Some(kwargs)).expect("call");
    assert_eq!(rendered.to_string(), "hi Ada");
}

#[rstest]
fn register_macro_is_reusable(mut strict_env: Environment) {
    let macro_def = MacroDefinition {
        signature: "echo(text)".to_string(),
        body: "{{ text }}".to_string(),
    };
    register_macro(&mut strict_env, &macro_def, 0).expect("register");

    let template = "{{ echo('first') }} {{ echo('second') }}";
    let rendered = render_with(&strict_env, template).expect("render once");
    assert_eq!(rendered.trim(), "first second");

    // Re-render to ensure the cached macro value remains valid.
    let rendered_again = render_with(&strict_env, template).expect("render twice");
    assert_eq!(rendered_again.trim(), "first second");
}

#[rstest]
fn register_manifest_macros_validates_shape(mut strict_env: Environment) {
    let mut mapping = ManifestMap::new();
    mapping.insert(
        "macros".into(),
        ManifestValue::Array(vec![ManifestValue::from(42)]),
    );
    let doc = ManifestValue::Object(mapping);
    let err = register_manifest_macros(&doc, &mut strict_env).expect_err("shape error");
    assert!(
        err.to_string()
            .contains("macros must be a sequence of mappings"),
        "{err}"
    );
}

#[rstest]
fn register_manifest_macros_rejects_non_string_values(mut strict_env: Environment) {
    let mut macro_mapping = ManifestMap::new();
    macro_mapping.insert("signature".into(), ManifestValue::from("greet(name)"));
    macro_mapping.insert(
        "body".into(),
        ManifestValue::Number(serde_json::Number::from(42)),
    );
    let macros = ManifestValue::Array(vec![ManifestValue::Object(macro_mapping)]);
    let mut doc = ManifestMap::new();
    doc.insert("macros".into(), macros);
    let doc = ManifestValue::Object(doc);

    let err = register_manifest_macros(&doc, &mut strict_env)
        .expect_err("non-string macro body should fail");
    let msg = err.to_string();
    assert!(msg.contains("macros"), "unexpected error: {msg}");
}

#[test]
fn manifest_macros_with_non_string_keys_fail_to_parse() {
    let yaml = r#"
macros:
  - ? [not, string]
    : signature: "greet(name)"
      body: "Hello"
"#;
    let err = serde_saphyr::from_str::<ManifestValue>(yaml).expect_err("expected parse failure");
    let msg = err.to_string();
    assert!(
        msg.contains("expected string scalar") || msg.contains("key") || msg.contains("mapping"),
        "{msg}"
    );
}

#[rstest]
fn register_manifest_macros_requires_body(mut strict_env: Environment) {
    let mut macro_mapping = ManifestMap::new();
    macro_mapping.insert("signature".into(), ManifestValue::from("greet(name)"));
    let macros = ManifestValue::Array(vec![ManifestValue::Object(macro_mapping)]);
    let mut doc = ManifestMap::new();
    doc.insert("macros".into(), macros);
    let doc = ManifestValue::Object(doc);

    let err = register_manifest_macros(&doc, &mut strict_env).expect_err("missing macro body");
    assert!(err.to_string().contains("body"), "{err}");
}

#[rstest]
fn register_manifest_macros_supports_multiple(mut strict_env: Environment) {
    let yaml = serde_saphyr::from_str::<ManifestValue>(
        "macros:\n  - signature: \"greet(name)\"\n    body: |\n      Hello {{ name }}\n  - signature: \"shout(text)\"\n    body: |\n      {{ text | upper }}\n",
    )
    .expect("yaml value");
    register_manifest_macros(&yaml, &mut strict_env).expect("register");
    let rendered = render_with(&strict_env, "{{ shout(greet('netsuke')) }}").expect("render");
    assert_eq!(rendered.trim(), "HELLO NETSUKE");
}
