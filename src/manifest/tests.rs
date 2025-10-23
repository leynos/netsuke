//! Tests for manifest parsing and macro registration helpers.

use super::jinja_macros::{
    call_macro_value, parse_macro_name, register_macro, register_manifest_macros,
};
use super::*;
use crate::ast::{MacroDefinition, Recipe};
use anyhow::{Context, Result as AnyResult, anyhow, ensure};
use minijinja::{
    Environment,
    value::{Kwargs, Value},
};
use rstest::{fixture, rstest};
use std::fs;
use tempfile::tempdir;
use test_support::{EnvVarGuard, env_lock::EnvLock, hash, http};

struct CurrentDirGuard {
    original: std::path::PathBuf,
    _lock: EnvLock,
}

impl CurrentDirGuard {
    fn change_to(path: &std::path::Path) -> AnyResult<Self> {
        let lock = EnvLock::acquire();
        let original = std::env::current_dir().context("capture current working directory")?;
        std::env::set_current_dir(path)
            .with_context(|| format!("switch to working directory {}", path.display()))?;
        Ok(Self {
            original,
            _lock: lock,
        })
    }
}

impl Drop for CurrentDirGuard {
    fn drop(&mut self) {
        if let Err(err) = std::env::set_current_dir(&self.original) {
            tracing::warn!("failed to restore working directory: {err}");
        }
    }
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
#[case("greet", "include parameter list")]
#[case("(name)", "missing an identifier")]
#[case("   ", "missing an identifier")]
fn parse_macro_name_errors(#[case] signature: &str, #[case] message: &str) -> AnyResult<()> {
    match parse_macro_name(signature) {
        Ok(name) => Err(anyhow!(
            "expected parse_macro_name to fail for {signature:?} but succeeded with {name}"
        )),
        Err(err) => {
            ensure!(
                err.to_string().contains(message),
                "expected error to contain {message:?}, got {err:?}"
            );
            Ok(())
        }
    }
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
) -> AnyResult<()> {
    let macro_def = MacroDefinition {
        signature: signature.to_owned(),
        body: body.to_owned(),
    };
    register_macro(&mut strict_env, &macro_def, 0)?;
    let rendered = render_with(&strict_env, template)?;
    ensure!(rendered == expected, "expected {expected}, got {rendered}");
    Ok(())
}

#[rstest]
fn call_macro_value_supports_kwargs(mut strict_env: Environment) -> AnyResult<()> {
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
    let kwargs = Kwargs::from_iter([(String::from("name"), Value::from("Ada"))]);
    let rendered = call_macro_value(&state, &value, &[], Some(kwargs))?;
    ensure!(
        rendered.to_string() == "hi Ada",
        "unexpected rendered output"
    );
    Ok(())
}

#[rstest]
fn register_macro_is_reusable(mut strict_env: Environment) -> AnyResult<()> {
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

#[rstest]
fn register_manifest_macros_validates_shape(mut strict_env: Environment) -> AnyResult<()> {
    let mut mapping = ManifestMap::new();
    mapping.insert(
        "macros".into(),
        ManifestValue::Array(vec![ManifestValue::from(42)]),
    );
    let doc = ManifestValue::Object(mapping);
    match register_manifest_macros(&doc, &mut strict_env) {
        Ok(()) => Err(anyhow!("expected error for non-mapping macro entry")),
        Err(err) => {
            ensure!(
                err.to_string()
                    .contains("macros must be a sequence of mappings"),
                "{err}"
            );
            Ok(())
        }
    }
}

#[rstest]
fn register_manifest_macros_rejects_non_string_values(
    mut strict_env: Environment,
) -> AnyResult<()> {
    let mut macro_mapping = ManifestMap::new();
    macro_mapping.insert("signature".into(), ManifestValue::from("greet(name)"));
    macro_mapping.insert(
        "body".into(),
        ManifestValue::Number(serde_json::Number::from(42)),
    );
    let macros = ManifestValue::Array(vec![ManifestValue::Object(macro_mapping)]);
    let mut doc = ManifestMap::new();
    doc.insert("macros".into(), macros);
    let doc_value = ManifestValue::Object(doc);

    match register_manifest_macros(&doc_value, &mut strict_env) {
        Ok(()) => Err(anyhow!(
            "register_manifest_macros should fail when macro body is not a string"
        )),
        Err(err) => {
            let msg = err.to_string();
            ensure!(msg.contains("macros"), "unexpected error: {msg}");
            Ok(())
        }
    }
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
fn register_manifest_macros_requires_body(mut strict_env: Environment) -> AnyResult<()> {
    let mut macro_mapping = ManifestMap::new();
    macro_mapping.insert("signature".into(), ManifestValue::from("greet(name)"));
    let macros = ManifestValue::Array(vec![ManifestValue::Object(macro_mapping)]);
    let mut doc = ManifestMap::new();
    doc.insert("macros".into(), macros);
    let doc_value = ManifestValue::Object(doc);

    match register_manifest_macros(&doc_value, &mut strict_env) {
        Ok(()) => Err(anyhow!("expected missing macro body to trigger an error")),
        Err(err) => {
            ensure!(err.to_string().contains("body"), "{err}");
            Ok(())
        }
    }
}

#[rstest]
fn register_manifest_macros_supports_multiple(mut strict_env: Environment) -> AnyResult<()> {
    let yaml = serde_saphyr::from_str::<ManifestValue>(
        "macros:\n  - signature: \"greet(name)\"\n    body: |\n      Hello {{ name }}\n  - signature: \"shout(text)\"\n    body: |\n      {{ text | upper }}\n",
    )?;
    register_manifest_macros(&yaml, &mut strict_env)?;
    let rendered = render_with(&strict_env, "{{ shout(greet('netsuke')) }}")?;
    ensure!(rendered.trim() == "HELLO NETSUKE");
    Ok(())
}

#[rstest]
fn from_path_uses_manifest_directory_for_caches() -> AnyResult<()> {
    let temp = tempdir()?;
    let workspace = temp.path().join("workspace");
    fs::create_dir_all(&workspace)?;
    let outside = temp.path().join("outside");
    fs::create_dir_all(&outside)?;
    let manifest_path = workspace.join("Netsukefile");

    let (url, server) = match http::spawn_http_server("workspace-body") {
        Ok(pair) => pair,
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            tracing::warn!(
                "Skipping from_path_uses_manifest_directory_for_caches: cannot bind HTTP listener ({err})"
            );
            return Ok(());
        }
        Err(err) => return Err(err.into()),
    };
    let manifest_yaml = concat!(
        "netsuke_version: \"1.0.0\"\n",
        "targets:\n",
        "  - name: fetch\n",
        "    vars:\n",
        "      url: \"{{ env('NETSUKE_MANIFEST_URL') }}\"\n",
        "    command: \"{{ fetch(url, cache=true) }}\"\n",
    );
    fs::write(&manifest_path, manifest_yaml)?;

    let _cwd_guard = CurrentDirGuard::change_to(&outside)?;
    let _url_guard = EnvVarGuard::set("NETSUKE_MANIFEST_URL", &url);

    let manifest = super::from_path(&manifest_path)?;
    if let Err(err) = server.join() {
        return Err(anyhow!("join server thread panicked: {err:?}"));
    }

    let first_target = manifest.targets.first().context("target missing")?;
    match &first_target.recipe {
        Recipe::Command { command } => anyhow::ensure!(
            command == "workspace-body",
            "unexpected recipe output: {command}"
        ),
        other => anyhow::bail!("expected command recipe, got {other:?}"),
    }

    let cache_key = hash::sha256_hex(url.as_bytes());
    let cache_path = workspace.join(".netsuke").join("fetch").join(cache_key);
    anyhow::ensure!(
        cache_path.exists(),
        "cache file should be created inside the manifest workspace"
    );
    anyhow::ensure!(
        !outside.join(".netsuke").exists(),
        "outside working directory must not receive cache data"
    );

    Ok(())
}
