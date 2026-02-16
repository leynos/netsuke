//! Tests for the YAML-first manifest pipeline: parse YAML, expand foreach/when,
//! then render Jinja only in string values.

use anyhow::{Context, Result, bail, ensure};
use netsuke::ast::{NetsukeManifest, Recipe, StringOrList, Target};
use netsuke::manifest::{self, ManifestError};
use rstest::{fixture, rstest};
use std::error::Error as StdError;
use test_support::{EnvVarGuard, env_lock::EnvLock, manifest::manifest_yaml};

/// Domain-specific environment variables exercised by manifest tests.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum EnvVar {
    /// Environment variable populated to verify successful interpolation.
    TestEnv,
    /// Environment variable intentionally absent to surface diagnostics.
    TestEnvMissing,
}

/// Manifest fields asserted repeatedly within the test suite.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum FieldName {
    /// Target output name field.
    Name,
    /// Recipe source files field.
    Sources,
    /// Prerequisite dependencies field.
    Deps,
    /// Order-only dependencies field.
    OrderOnlyDeps,
    /// Nested rule reference field.
    Rule,
}

impl EnvVar {
    const fn as_str(self) -> &'static str {
        match self {
            Self::TestEnv => "NETSUKE_TEST_ENV",
            Self::TestEnvMissing => "NETSUKE_TEST_ENV_MISSING",
        }
    }
}

impl FieldName {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Sources => "sources",
            Self::Deps => "deps",
            Self::OrderOnlyDeps => "order_only_deps",
            Self::Rule => "rule",
        }
    }
}

const ENV_YAML: &str = "targets:\n  - name: env\n    command: echo {{ env('NETSUKE_TEST_ENV') }}\n";
const ENV_MISSING_YAML: &str =
    "targets:\n  - name: env_missing\n    command: echo {{ env('NETSUKE_TEST_ENV_MISSING') }}\n";

#[fixture]
fn env_lock() -> EnvLock {
    EnvLock::acquire()
}

fn assert_string_or_list_eq(actual: &StringOrList, expected: &str, field: FieldName) -> Result<()> {
    match actual {
        StringOrList::String(value) => {
            ensure!(
                value == expected,
                "expected {} to equal '{expected}', got '{value}'",
                field.as_str()
            );
            Ok(())
        }
        StringOrList::List(list) if list.len() == 1 => {
            let first = list
                .first()
                .context("single-item list should contain exactly one element")?;
            ensure!(
                first == expected,
                "expected {} to equal '{expected}', got '{first}'",
                field.as_str()
            );
            Ok(())
        }
        other => bail!(
            "expected {} to be a string or single-item list, got {other:?}",
            field.as_str()
        ),
    }
}

fn assert_string_or_list_eq_list(
    actual: &StringOrList,
    expected: &[String],
    field: FieldName,
) -> Result<()> {
    match actual {
        StringOrList::List(list) => {
            ensure!(
                list == expected,
                "expected {} to equal {:?}, got {:?}",
                field.as_str(),
                expected,
                list
            );
            Ok(())
        }
        other => bail!("expected {} to be a list, got {other:?}", field.as_str()),
    }
}

fn extract_target_field<F>(manifest: &NetsukeManifest, extract: F) -> Result<Vec<String>>
where
    F: Fn(&Target) -> Result<String>,
{
    manifest.targets.iter().map(extract).collect()
}

fn extract_target_names(manifest: &NetsukeManifest) -> Result<Vec<String>> {
    extract_target_field(manifest, |target| match &target.name {
        StringOrList::String(name) => Ok(name.clone()),
        other => bail!("expected target name to be string, got {other:?}"),
    })
}

fn extract_target_commands(manifest: &NetsukeManifest) -> Result<Vec<String>> {
    extract_target_field(manifest, |target| match &target.recipe {
        Recipe::Command { command } => Ok(command.clone()),
        other => bail!("expected command recipe, got {other:?}"),
    })
}

fn format_error_message(error: &(dyn StdError + 'static)) -> String {
    error.to_string()
}

#[rstest]
fn renders_global_vars() -> Result<()> {
    let yaml = manifest_yaml(
        "vars:\n  who: world\ntargets:\n  - name: hello\n    command: echo {{ who }}\n",
    );

    let manifest = manifest::from_str(&yaml)?;
    let first = manifest
        .targets
        .first()
        .context("manifest should contain at least one target")?;
    let Recipe::Command { command } = &first.recipe else {
        bail!("expected command recipe, got {:?}", first.recipe);
    };
    ensure!(command == "echo world", "unexpected command: {command}");
    Ok(())
}

#[rstest]
fn renders_env_function(env_lock: EnvLock) -> Result<()> {
    let _env_lock = env_lock;
    let _guard = EnvVarGuard::set(EnvVar::TestEnv.as_str(), "42");
    let yaml = manifest_yaml(ENV_YAML);

    let manifest = manifest::from_str(&yaml)?;
    let first = manifest
        .targets
        .first()
        .context("manifest should contain at least one target")?;
    let Recipe::Command { command } = &first.recipe else {
        bail!("expected command recipe, got {:?}", first.recipe);
    };
    ensure!(command == "echo 42", "unexpected command: {command}");
    Ok(())
}

#[rstest]
fn renders_env_function_missing_var(env_lock: EnvLock) -> Result<()> {
    let _env_lock = env_lock;
    let name = EnvVar::TestEnvMissing;
    let _guard = EnvVarGuard::remove(name.as_str());
    let yaml = manifest_yaml(ENV_MISSING_YAML);

    match manifest::from_str(&yaml) {
        Ok(parsed) => bail!("expected missing env var to error, got manifest {parsed:?}"),
        Err(err) => {
            if let Some(manifest_err) = err.downcast_ref::<ManifestError>() {
                ensure!(
                    matches!(manifest_err, ManifestError::Parse { .. }),
                    "expected ManifestError::Parse, got {manifest_err:?}"
                );
            } else {
                ensure!(
                    err.chain().any(|source| {
                        format!("{source:?}").contains("UndefinedError")
                            && source.to_string().contains(name.as_str())
                    }),
                    "unexpected error type or message: {err:?}"
                );
            }
            let msg = format!("{err:?}");
            ensure!(
                msg.contains(name.as_str()),
                "missing env var name not present in message: {msg}"
            );
            Ok(())
        }
    }
}

#[rstest]
fn undefined_variable_errors() -> Result<()> {
    let yaml = manifest_yaml("targets:\n  - name: hello\n    command: echo {{ missing }}\n");
    ensure!(
        manifest::from_str(&yaml).is_err(),
        "expected undefined variable to raise an error"
    );
    Ok(())
}

#[rstest]
fn registers_manifest_macros() -> Result<()> {
    let yaml = manifest_yaml(concat!(
        "macros:\n",
        "  - signature: \"greet(name)\"\n",
        "    body: |-\n",
        "      Hello {{ name }}!\n",
        "  - signature: \"shout(text)\"\n",
        "    body: |-\n",
        "      {{ text | upper }}\n",
        "targets:\n",
        "  - name: greet\n",
        "    command: \"{{ shout(greet('world')) }}\"\n",
    ));

    let manifest = manifest::from_str(&yaml)?;
    let target = manifest
        .targets
        .first()
        .context("manifest should contain at least one target")?;
    match &target.recipe {
        Recipe::Command { command } => {
            ensure!(command == "HELLO WORLD!", "unexpected command: {command}");
        }
        other => bail!("expected command recipe, got {other:?}"),
    }
    Ok(())
}

#[rstest]
#[case(
    concat!(
        "macros:\n",
        "  - signature: \"no_args()\"\n",
        "    body: |-\n",
        "      ready\n",
    ),
    "{{ no_args() }}",
    "ready",
)]
#[case(
    concat!(
        "macros:\n",
        "  - signature: \"defaulted(name='world')\"\n",
        "    body: |-\n",
        "      Hi {{ name }}\n",
    ),
    "{{ defaulted() }}",
    "Hi world",
)]
#[case(
    concat!(
        "macros:\n",
        "  - signature: \"joiner(items)\"\n",
        "    body: |-\n",
        "      {{ items | join(',') }}\n",
    ),
    "{{ joiner(['a', 'b', 'c']) }}",
    "a,b,c",
)]
#[case(
    concat!(
        "macros:\n",
        "  - signature: \"show(name, excited=false)\"\n",
        "    body: |-\n",
        "      {{ name ~ ('!' if excited else '') }}\n",
    ),
    "{{ show('Netsuke', excited=true) }}",
    "Netsuke!",
)]
fn registers_manifest_macro_argument_variants(
    #[case] macros_block: &str,
    #[case] expression: &str,
    #[case] expected: &str,
) -> Result<()> {
    let rendered_command = expression.replace('"', "\\\"");
    let yaml = manifest_yaml(&format!(
        "{macros_block}targets:\n  - name: test\n    command: \"{rendered_command}\"\n",
    ));

    let manifest = manifest::from_str(&yaml)?;
    let target = manifest
        .targets
        .first()
        .context("manifest should contain at least one target")?;
    match &target.recipe {
        Recipe::Command { command } => {
            ensure!(command == expected, "unexpected command: {command}");
        }
        other => bail!("expected command recipe, got {other:?}"),
    }
    Ok(())
}

#[rstest]
fn manifest_macro_with_missing_signature_errors() -> Result<()> {
    let yaml = manifest_yaml(concat!(
        "macros:\n",
        "  - body: |\n",
        "      hi\n",
        "targets:\n",
        "  - name: noop\n",
        "    command: noop\n",
    ));

    let err = manifest::from_str(&yaml)
        .err()
        .context("missing macro signature should raise an error")?;
    let msg = format!("{err:?}");
    ensure!(msg.contains("signature"), "error message: {msg}");
    Ok(())
}

#[rstest]
fn manifest_macro_with_missing_body_errors() -> Result<()> {
    let yaml = manifest_yaml(concat!(
        "macros:\n",
        "  - signature: \"greet(name)\"\n",
        "targets:\n",
        "  - name: noop\n",
        "    command: noop\n",
    ));

    let err = manifest::from_str(&yaml)
        .err()
        .context("missing macro body should raise an error")?;
    let msg = format!("{err:?}");
    ensure!(msg.contains("body"), "error message: {msg}");
    Ok(())
}

#[rstest]
fn syntax_error_errors() -> Result<()> {
    let yaml = manifest_yaml("targets:\n  - name: hello\n    command: echo {{ who\n");
    ensure!(manifest::from_str(&yaml).is_err(), "expected syntax error");
    Ok(())
}

#[rstest]
#[case(true, "echo on")]
#[case(false, "echo off")]
fn renders_if_blocks(#[case] flag: bool, #[case] expected: &str) -> Result<()> {
    let cmd = "{% if flag %}echo on{% else %}echo off{% endif %}";
    let yaml = manifest_yaml(&format!(
        concat!(
            "vars:\n",
            "  flag: {flag}\n",
            "targets:\n",
            "  - name: test\n",
            "    command: \"{cmd}\"\n",
        ),
        flag = flag,
        cmd = cmd,
    ));

    let manifest = manifest::from_str(&yaml)?;
    let first = manifest
        .targets
        .first()
        .context("manifest should contain at least one target")?;
    let Recipe::Command { command } = &first.recipe else {
        bail!("expected command recipe, got {:?}", first.recipe);
    };
    ensure!(command == expected, "unexpected command: {command}");
    Ok(())
}

#[rstest]
#[case(
    concat!(
        "targets:\n",
        "  - foreach:\n",
        "      - a\n",
        "      - b\n",
        "    name: '{{ item }}'\n",
        "    command: \"echo '{{ item }}'\"\n",
    ),
    vec!["a", "b"],
    vec!["echo 'a'", "echo 'b'"],
)]
#[case(
    concat!(
        "targets:\n",
        "  - foreach: \"['x', 'y']\"\n",
        "    name: '{{ index }}:{{ item }}'\n",
        "    command: 'echo {{ index }} {{ item }}'\n",
    ),
    vec!["0:x", "1:y"],
    vec!["echo 0 x", "echo 1 y"],
)]
fn expands_foreach_with_item_and_index(
    #[case] yaml_body: &str,
    #[case] expected_names: Vec<&str>,
    #[case] expected_commands: Vec<&str>,
) -> Result<()> {
    let yaml = manifest_yaml(yaml_body);
    let manifest = manifest::from_str(&yaml)?;

    let names = extract_target_names(&manifest)?;
    ensure!(
        names
            == expected_names
                .iter()
                .copied()
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>(),
        "unexpected names: {names:?}"
    );

    let commands = extract_target_commands(&manifest)?;
    ensure!(
        commands
            == expected_commands
                .iter()
                .copied()
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>(),
        "unexpected commands: {commands:?}"
    );
    Ok(())
}

#[rstest]
#[case(
    "[]",
    "",
    "no targets should be generated for empty foreach list",
    true
)]
#[case(
    "['a', 'b']",
    "false",
    "no targets should be generated when condition is always false",
    true
)]
#[case(
    "[]",
    "",
    "no targets should be generated for empty foreach list (typed)",
    false
)]
fn no_targets_generated_scenarios(
    #[case] foreach_value: &str,
    #[case] when_clause: &str,
    #[case] assertion_message: &str,
    #[case] quoted_foreach: bool,
) -> Result<()> {
    let when_line = if when_clause.is_empty() {
        String::new()
    } else {
        format!("    when: \"{when_clause}\"\n")
    };

    let foreach_lit = if quoted_foreach {
        format!("\"{foreach_value}\"")
    } else {
        foreach_value.to_owned()
    };

    let yaml = manifest_yaml(&format!(
        "targets:\n  - foreach: {foreach_lit}\n{when_line}    name: '{{ item }}'\n    command: 'echo {{ item }}'\n",
    ));

    let manifest = manifest::from_str(&yaml)?;
    ensure!(manifest.targets.is_empty(), "{assertion_message}");
    Ok(())
}

#[rstest]
fn expands_single_item_foreach_targets() -> Result<()> {
    let yaml = manifest_yaml(
        "targets:\n  - foreach:\n      - only\n    name: '{{ item }}'\n    command: \"echo '{{ item }}'\"\n",
    );

    let manifest = manifest::from_str(&yaml)?;
    ensure!(
        manifest.targets.len() == 1,
        "exactly one target should be generated for single-item foreach list"
    );
    let first = manifest
        .targets
        .first()
        .context("manifest should contain at least one target")?;
    assert_string_or_list_eq(&first.name, "only", FieldName::Name)?;
    let Recipe::Command { command } = &first.recipe else {
        bail!("expected command recipe, got {:?}", first.recipe);
    };
    ensure!(command == "echo 'only'", "unexpected command: {command}");
    Ok(())
}

#[rstest]
#[case("1", true)]
#[case("1", false)]
fn foreach_non_iterable_errors(#[case] val: &str, #[case] quoted: bool) -> Result<()> {
    let foreach = if quoted {
        format!("\"{val}\"")
    } else {
        val.to_owned()
    };
    let yaml = manifest_yaml(&format!(
        "targets:\n  - foreach: {foreach}\n    name: 'a'\n    command: 'echo a'\n",
    ));

    ensure!(
        manifest::from_str(&yaml).is_err(),
        "non-iterable foreach should raise an error"
    );
    Ok(())
}

#[rstest]
fn foreach_vars_must_be_mapping() -> Result<()> {
    let yaml = manifest_yaml(
        "targets:\n  - foreach: ['a']\n    vars: 1\n    name: 'x'\n    command: 'echo x'\n",
    );

    let err = manifest::from_str(&yaml)
        .err()
        .context("vars must be a mapping")?;
    ensure!(
        err.chain()
            .map(format_error_message)
            .any(|msg| msg.contains("Target `vars` must be an object")),
        "unexpected error: {err}"
    );
    Ok(())
}

#[rstest]
fn foreach_when_filters_items() -> Result<()> {
    let yaml = manifest_yaml(
        "targets:\n  - foreach:\n      - a\n      - skip\n      - b\n    when: item != 'skip'\n    name: '{{ item }}'\n    command: \"echo '{{ item }}'\"\n",
    );

    let manifest = manifest::from_str(&yaml)?;
    ensure!(manifest.targets.len() == 2, "unexpected target count");
    let names = extract_target_names(&manifest)?;
    ensure!(names == ["a", "b"], "unexpected names: {names:?}");
    let commands = extract_target_commands(&manifest)?;
    ensure!(
        commands == ["echo 'a'", "echo 'b'"],
        "unexpected commands: {commands:?}"
    );
    Ok(())
}

#[rstest]
fn renders_target_fields_command() -> Result<()> {
    let yaml = manifest_yaml(
        "vars:\n  base: base\n  \ntargets:\n  - foreach:\n      - 1\n    vars:\n      local: '{{ base }}{{ item }}'\n    name: '{{ local }}'\n    sources: ['{{ local }}.src']\n    deps: ['{{ local }}.dep']\n    order_only_deps: ['{{ local }}.ord']\n    command: \"echo '{{ local }}'\"\n",
    );

    let manifest = manifest::from_str(&yaml)?;
    let target = manifest
        .targets
        .first()
        .context("manifest should contain at least one target")?;
    assert_string_or_list_eq(&target.name, "base1", FieldName::Name)?;
    assert_string_or_list_eq_list(
        &target.sources,
        &["base1.src".to_owned()],
        FieldName::Sources,
    )?;
    assert_string_or_list_eq_list(&target.deps, &["base1.dep".to_owned()], FieldName::Deps)?;
    assert_string_or_list_eq_list(
        &target.order_only_deps,
        &["base1.ord".to_owned()],
        FieldName::OrderOnlyDeps,
    )?;
    let Recipe::Command { command } = &target.recipe else {
        bail!("expected command recipe, got {:?}", target.recipe);
    };
    ensure!(command == "echo 'base1'", "unexpected command: {command}");
    Ok(())
}

#[rstest]
#[case(
    "script",
    "run base.sh",
    "vars:\n  base: base\n  \ntargets:\n  - name: script\n    vars:\n      path: '{{ base }}.sh'\n    script: 'run {{ path }}'\n"
)]
#[case(
    "rule",
    "base-rule",
    "vars:\n  base: base\nrules:\n  - name: base-rule\n    command: echo hi\n\ntargets:\n  - name: use-rule\n    rule: '{{ base }}-rule'\n"
)]
fn renders_target_fields_recipe_types(
    #[case] recipe_type: &str,
    #[case] expected_value: &str,
    #[case] yaml_body: &str,
) -> Result<()> {
    let yaml = manifest_yaml(yaml_body);
    let manifest = manifest::from_str(&yaml)?;
    let target = manifest
        .targets
        .first()
        .context("manifest should contain at least one target")?;
    match (recipe_type, &target.recipe) {
        ("script", Recipe::Script { script }) => {
            ensure!(script == expected_value, "unexpected script: {script}");
        }
        ("rule", Recipe::Rule { rule }) => {
            assert_string_or_list_eq(rule, expected_value, FieldName::Rule)?;
        }
        ("script", recipe) => bail!("expected script recipe, got {recipe:?}"),
        ("rule", recipe) => bail!("expected rule recipe, got {recipe:?}"),
        (other, _) => bail!("unexpected recipe type: {other}"),
    }
    Ok(())
}

#[rstest]
fn render_target_missing_var_errors() -> Result<()> {
    let yaml = manifest_yaml(
        "targets:\n  - name: test\n    sources: ['{{ missing }}']\n    command: echo hi\n",
    );

    ensure!(
        manifest::from_str(&yaml).is_err(),
        "expected missing var error"
    );
    Ok(())
}

#[rstest]
fn undefined_in_if_errors() -> Result<()> {
    let yaml = manifest_yaml(
        "targets:\n  - name: test\n    command: \"{% if missing %}echo hi{% endif %}\"\n",
    );

    ensure!(
        manifest::from_str(&yaml).is_err(),
        "expected undefined var error"
    );
    Ok(())
}
