//! Tests for manifest parsing and macro registration helpers.

use super::*;
use anyhow::Result as AnyResult;
use minijinja::Environment;
use rstest::{fixture, rstest};
use serde_yml::value::Mapping;

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
fn register_manifest_macros_validates_shape(mut strict_env: Environment) {
    let mut mapping = Mapping::new();
    mapping.insert(
        YamlValue::from("macros"),
        YamlValue::from(vec![YamlValue::from(42)]),
    );
    let doc = YamlValue::Mapping(mapping);
    let err = register_manifest_macros(&doc, &mut strict_env).expect_err("shape error");
    assert!(
        err.to_string()
            .contains("macros must be a sequence of mappings"),
        "{err}"
    );
}

#[rstest]
fn register_manifest_macros_supports_multiple(mut strict_env: Environment) {
    let yaml = serde_yml::from_str::<YamlValue>(
        "macros:\n  - signature: \"greet(name)\"\n    body: |\n      Hello {{ name }}\n  - signature: \"shout(text)\"\n    body: |\n      {{ text | upper }}\n",
    )
    .expect("yaml value");
    register_manifest_macros(&yaml, &mut strict_env).expect("register");
    let rendered = render_with(&strict_env, "{{ shout(greet('netsuke')) }}").expect("render");
    assert_eq!(rendered.trim(), "HELLO NETSUKE");
}
