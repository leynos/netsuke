//! Manifest loading helpers.
//!
//! This module parses a `Netsukefile` without relying on a global Jinja
//! preprocessing pass. The YAML is parsed first and Jinja expressions are
//! evaluated only within string values or the `foreach` and `when` keys. It
//! exposes `env()` to read environment variables and `glob()` to expand
//! filesystem patterns during template evaluation. Both helpers fail fast when
//! inputs are missing or patterns are invalid.

use crate::ast::{MacroDefinition, NetsukeManifest};
use anyhow::{Context, Result};
use minijinja::{
    Environment, Error, ErrorKind, State, UndefinedBehavior,
    value::{Rest, Value},
};
use serde_yml::Value as YamlValue;
use std::{fs, path::Path};

mod diagnostics;
mod expand;
mod glob;
mod hints;
mod render;

pub use diagnostics::{ManifestError, map_yaml_error};
pub use glob::glob_paths;

pub use expand::expand_foreach;
pub use render::render_manifest;

/// Resolve the value of an environment variable for the `env()` Jinja helper.
///
/// Returns the variable's value or a structured error that mirrors Jinja's
/// failure modes, ensuring templates halt when a variable is missing or not
/// valid UTF-8.
///
/// # Examples
///
/// The [`EnvLock`](test_support::env_lock::EnvLock) guard serialises access to
/// the process environment so tests do not interfere with each other.
///
/// ```rust,ignore
/// use test_support::env_lock::EnvLock;
/// let _guard = EnvLock::acquire();
/// std::env::set_var("FOO", "bar");
/// assert_eq!(env("FOO").unwrap(), "bar");
/// std::env::remove_var("FOO");
/// ```
fn env_var(name: &str) -> std::result::Result<String, Error> {
    match std::env::var(name) {
        Ok(val) => Ok(val),
        Err(std::env::VarError::NotPresent) => Err(Error::new(
            ErrorKind::UndefinedError,
            format!("environment variable '{name}' is not set"),
        )),
        Err(std::env::VarError::NotUnicode(_)) => Err(Error::new(
            ErrorKind::InvalidOperation,
            format!("environment variable '{name}' is set but contains invalid UTF-8"),
        )),
    }
}

fn parse_macro_name(signature: &str) -> Result<String> {
    let trimmed = signature.trim();
    if trimmed.is_empty() {
        return Err(anyhow::anyhow!(
            "macro signature '{signature}' is missing an identifier"
        ));
    }
    let Some((name, _rest)) = trimmed.split_once('(') else {
        return Err(anyhow::anyhow!(
            "macro signature '{signature}' must include parameter list"
        ));
    };
    let name = name.trim();
    if name.is_empty() {
        return Err(anyhow::anyhow!(
            "macro signature '{signature}' is missing an identifier"
        ));
    }
    Ok(name.to_string())
}

fn register_macro(env: &mut Environment, macro_def: &MacroDefinition, index: usize) -> Result<()> {
    let name = parse_macro_name(&macro_def.signature)?;
    let template_name = format!("__manifest_macro_{index}_{name}");
    let template_source = format!(
        "{{% macro {} %}}{}{{% endmacro %}}",
        macro_def.signature, macro_def.body
    );

    env.add_template_owned(template_name.clone(), template_source)
        .with_context(|| format!("compile macro '{name}'"))?;

    env.add_function(name.clone(), make_macro_fn(template_name, name));
    Ok(())
}

fn register_manifest_macros(doc: &YamlValue, env: &mut Environment) -> Result<()> {
    let Some(macros) = doc.get("macros").cloned() else {
        return Ok(());
    };

    let defs: Vec<MacroDefinition> = serde_yml::from_value(macros)
        .context("macros must be a sequence of mappings with string signature/body")?;

    for (idx, def) in defs.into_iter().enumerate() {
        let macro_def = MacroDefinition {
            signature: def.signature,
            body: def.body,
        };
        let macro_name = macro_def.signature.clone();
        register_macro(env, &macro_def, idx)
            .with_context(|| format!("register macro '{macro_name}'"))?;
    }
    Ok(())
}

fn make_macro_fn(
    template_name: String,
    macro_name: String,
) -> impl Fn(&State, Rest<Value>) -> Result<Value, Error> {
    move |state, Rest(args)| {
        let template = state.env().get_template(&template_name)?;
        let macro_state = template.eval_to_state(())?;
        let value = macro_state.lookup(&macro_name).ok_or_else(|| {
            Error::new(
                ErrorKind::InvalidOperation,
                format!("macro '{macro_name}' not defined in template '{template_name}'"),
            )
        })?;
        value.call(&macro_state, &args)
    }
}

/// Parse a manifest string using Jinja for value templating.
///
/// The input YAML must be valid on its own. Jinja expressions are evaluated
/// only inside recognised string fields and the `foreach` and `when` keys.
///
/// # Errors
///
/// Returns an error if YAML parsing or Jinja evaluation fails.
fn from_str_named(yaml: &str, name: &str) -> Result<NetsukeManifest> {
    let mut doc: YamlValue = serde_yml::from_str(yaml).map_err(|e| ManifestError::Parse {
        source: map_yaml_error(e, yaml, name),
    })?;

    let mut jinja = Environment::new();
    jinja.set_undefined_behavior(UndefinedBehavior::Strict);
    // Expose custom helpers to templates.
    jinja.add_function("env", |name: String| env_var(&name));
    jinja.add_function("glob", |pattern: String| glob_paths(&pattern));
    let _stdlib_state = crate::stdlib::register(&mut jinja);

    if let Some(vars) = doc.get("vars").and_then(|v| v.as_mapping()).cloned() {
        for (k, v) in vars {
            let key = k
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("non-string key in vars mapping: {k:?}"))?
                .to_string();
            jinja.add_global(key, Value::from_serialize(v));
        }
    }

    register_manifest_macros(&doc, &mut jinja)?;

    expand_foreach(&mut doc, &jinja)?;

    let manifest: NetsukeManifest =
        serde_yml::from_value(doc).map_err(|e| ManifestError::Parse {
            source: map_yaml_error(e, yaml, name),
        })?;

    render_manifest(manifest, &jinja)
}

/// Parse a manifest string using Jinja for value templating.
///
/// The input YAML must be valid on its own. Jinja expressions are evaluated
/// only inside recognised string fields and the `foreach` and `when` keys.
///
/// # Errors
///
/// Returns an error if YAML parsing or Jinja evaluation fails.
pub fn from_str(yaml: &str) -> Result<NetsukeManifest> {
    from_str_named(yaml, "Netsukefile")
}

/// Load a [`NetsukeManifest`] from the given file path.
///
/// # Errors
///
/// Returns an error if the file cannot be read or the YAML fails to parse.
pub fn from_path(path: impl AsRef<Path>) -> Result<NetsukeManifest> {
    let path_ref = path.as_ref();
    let data = fs::read_to_string(path_ref)
        .with_context(|| format!("failed to read {}", path_ref.display()))?;
    from_str_named(&data, &path_ref.display().to_string())
}

#[cfg(test)]
mod tests {
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
}
