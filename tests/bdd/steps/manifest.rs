//! Step definitions for manifest parsing scenarios.

use crate::bdd::fixtures::{RefCellOptionExt, with_world};
use crate::bdd::types::{
    CommandText, DepName, EnvVarKey, EnvVarValue, ErrorPattern, MacroSignature, ManifestPath,
    NamesList, RuleName, ScriptText, SourcePath, TargetName, VersionString,
};
use anyhow::{Context, Result, bail, ensure};
use netsuke::{
    ast::{Recipe, StringOrList},
    manifest,
};
use rstest_bdd_macros::{given, then, when};
use std::{collections::BTreeSet, convert::TryFrom, ffi::OsStr};
use test_support::display_error_chain;
use test_support::env::{remove_var, set_var};

const INDEX_KEY: &str = "index";

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

fn get_string_from_string_or_list(value: &StringOrList, field_name: &str) -> Result<String> {
    match value {
        StringOrList::String(s) => Ok(s.clone()),
        StringOrList::List(list) => {
            ensure!(
                list.len() == 1,
                "Expected String or single-item List for {field_name}, got list of length {}",
                list.len()
            );
            list.first()
                .cloned()
                .with_context(|| format!("{field_name} list unexpectedly empty"))
        }
        StringOrList::Empty => {
            bail!("Expected String or single-item List for {field_name}, got empty value")
        }
    }
}

fn parse_manifest_inner(path: &ManifestPath) {
    with_world(|world| match manifest::from_path(path.as_str()) {
        Ok(manifest) => {
            world.manifest.set_value(manifest);
            world.manifest_error.clear();
        }
        Err(e) => {
            world.manifest.clear_value();
            world.manifest_error.set(display_error_chain(e.as_ref()));
        }
    });
}

fn assert_manifest() -> Result<()> {
    with_world(|world| {
        ensure!(
            world.manifest.is_some(),
            "manifest should have been parsed successfully"
        );
        Ok(())
    })
}

fn assert_parsed() -> Result<()> {
    with_world(|world| {
        ensure!(
            world.manifest.is_some() || world.manifest_error.is_filled(),
            "manifest should have been parsed"
        );
        Ok(())
    })
}

fn assert_field_eq(
    context_name: &str,
    field_name: &str,
    actual: &str,
    expected: &str,
) -> Result<()> {
    ensure!(
        actual == expected,
        "expected {context_name} {field_name} '{expected}', got '{actual}'"
    );
    Ok(())
}

fn parse_env_token<I>(chars: &mut std::iter::Peekable<I>) -> String
where
    I: Iterator<Item = char>,
{
    chars.next();
    let mut name = String::new();
    for ch in chars.by_ref() {
        if ch == '}' {
            break;
        }
        name.push(ch);
    }
    std::env::var(&name).unwrap_or_else(|_| ["${", &name, "}"].concat())
}

fn expand_env(raw: &str) -> String {
    let mut out = String::new();
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '$' && chars.peek() == Some(&'{') {
            out.push_str(&parse_env_token(&mut chars));
        } else {
            out.push(c);
        }
    }
    out
}

fn assert_list_contains(value: &StringOrList, expected: &str) -> Result<()> {
    match value {
        StringOrList::List(list) => ensure!(
            list.iter().any(|entry| entry == expected),
            "missing {expected}"
        ),
        StringOrList::String(s) => ensure!(s == expected, "expected '{expected}', got '{s}'"),
        StringOrList::Empty => bail!("value is empty"),
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Given steps
// ---------------------------------------------------------------------------

#[given("the environment variable {key} is set to {value}")]
fn set_env_var_step(key: String, value: String) -> Result<()> {
    let key = EnvVarKey::new(key);
    let value = EnvVarValue::new(value);
    ensure!(
        !key.as_str().is_empty(),
        "environment variable name must not be empty"
    );
    let expanded = expand_env(value.as_str());
    let previous = set_var(key.as_str(), OsStr::new(&expanded));
    with_world(|world| {
        world.track_env_var(key.into_string(), previous);
    });
    Ok(())
}

#[given("the environment variable {key} is unset")]
fn unset_env_var_step(key: String) -> Result<()> {
    let key = EnvVarKey::new(key);
    ensure!(
        !key.as_str().is_empty(),
        "environment variable name must not be empty"
    );
    let previous = remove_var(key.as_str());
    with_world(|world| {
        world.track_env_var(key.into_string(), previous);
    });
    Ok(())
}

#[given("the manifest file {path} is parsed")]
fn given_parse_manifest(path: String) -> Result<()> {
    let path = ManifestPath::new(path);
    ensure!(
        !path.as_str().trim().is_empty(),
        "manifest path must not be an empty string"
    );
    parse_manifest_inner(&path);
    Ok(())
}

// ---------------------------------------------------------------------------
// When steps
// ---------------------------------------------------------------------------

#[when("the manifest file {path} is parsed")]
fn when_parse_manifest(path: String) -> Result<()> {
    let path = ManifestPath::new(path);
    ensure!(
        !path.as_str().trim().is_empty(),
        "manifest path must not be an empty string"
    );
    parse_manifest_inner(&path);
    Ok(())
}

#[when("the parsing result is checked")]
fn when_parsing_result_checked() -> Result<()> {
    assert_parsed()
}

#[when("the manifest is checked")]
fn when_manifest_checked() -> Result<()> {
    assert_manifest()
}

#[when("the version is checked")]
fn when_version_checked() -> Result<()> {
    assert_manifest()
}

#[when("the flags are checked")]
fn when_flags_checked() -> Result<()> {
    assert_manifest()
}

#[when("the rules are checked")]
fn when_rules_checked() -> Result<()> {
    assert_manifest()
}

// ---------------------------------------------------------------------------
// Then steps
// ---------------------------------------------------------------------------

#[then("the manifest version is {version}")]
fn manifest_version(version: String) -> Result<()> {
    let version = VersionString::new(version);
    with_world(|world| {
        let actual = world
            .manifest
            .with_ref(|m| m.netsuke_version.to_string())
            .context("manifest has not been parsed")?;
        assert_field_eq("manifest", "version", actual.as_str(), version.as_str())
    })
}

#[then("the first target name is {name}")]
fn first_target_name(name: String) -> Result<()> {
    let name = TargetName::new(name);
    with_world(|world| {
        let result = world.manifest.with_ref(|m| {
            let target = m.targets.first().context("missing target 1")?;
            let actual = get_string_from_string_or_list(&target.name, "name")?;
            ensure!(
                actual == name.as_str(),
                "expected target 1 name '{}', got '{actual}'",
                name
            );
            Ok(())
        });
        result.context("manifest has not been parsed")?
    })
}

#[then("the target {index:usize} is phony")]
fn target_is_phony(index: usize) -> Result<()> {
    ensure!(index > 0, "target index is 1-based");
    with_world(|world| {
        let result = world.manifest.with_ref(|m| {
            let target = m
                .targets
                .get(index - 1)
                .with_context(|| format!("missing target {index}"))?;
            ensure!(target.phony, "target {index} should be phony");
            Ok(())
        });
        result.context("manifest has not been parsed")?
    })
}

#[then("the target {index:usize} is always rebuilt")]
fn target_is_always(index: usize) -> Result<()> {
    ensure!(index > 0, "target index is 1-based");
    with_world(|world| {
        let result = world.manifest.with_ref(|m| {
            let target = m
                .targets
                .get(index - 1)
                .with_context(|| format!("missing target {index}"))?;
            ensure!(target.always, "target {index} should always build");
            Ok(())
        });
        result.context("manifest has not been parsed")?
    })
}

#[then("the target {index:usize} is not phony")]
fn target_not_phony(index: usize) -> Result<()> {
    ensure!(index > 0, "target index is 1-based");
    with_world(|world| {
        let result = world.manifest.with_ref(|m| {
            let target = m
                .targets
                .get(index - 1)
                .with_context(|| format!("missing target {index}"))?;
            ensure!(!target.phony, "target {index} should not be phony");
            Ok(())
        });
        result.context("manifest has not been parsed")?
    })
}

#[then("the target {index:usize} is not always rebuilt")]
fn target_not_always(index: usize) -> Result<()> {
    ensure!(index > 0, "target index is 1-based");
    with_world(|world| {
        let result = world.manifest.with_ref(|m| {
            let target = m
                .targets
                .get(index - 1)
                .with_context(|| format!("missing target {index}"))?;
            ensure!(!target.always, "target {index} should not always build");
            Ok(())
        });
        result.context("manifest has not been parsed")?
    })
}

#[then("the first action is phony")]
fn first_action_phony() -> Result<()> {
    with_world(|world| {
        let result = world.manifest.with_ref(|m| {
            let first = m
                .actions
                .first()
                .context("manifest does not contain any actions")?;
            ensure!(first.phony, "expected first action to be marked phony");
            Ok(())
        });
        result.context("manifest has not been parsed")?
    })
}

#[then("parsing the manifest fails")]
fn manifest_parse_error() -> Result<()> {
    with_world(|world| {
        ensure!(
            world.manifest_error.is_filled(),
            "expected manifest parsing to record an error"
        );
        Ok(())
    })
}

#[then("the error message contains {text}")]
fn manifest_error_contains(text: String) -> Result<()> {
    let text = ErrorPattern::new(text);
    with_world(|world| {
        let msg = world
            .manifest_error
            .get()
            .context("expected manifest parsing to produce an error")?;
        ensure!(
            msg.contains(text.as_str()),
            "expected parse error to contain '{}', but was '{msg}'",
            text
        );
        Ok(())
    })
}

#[then("the first rule name is {name}")]
fn first_rule_name(name: String) -> Result<()> {
    let name = RuleName::new(name);
    with_world(|world| {
        let result = world.manifest.with_ref(|m| {
            let rule = m
                .rules
                .first()
                .context("manifest does not contain any rules")?;
            assert_field_eq("first rule", "name", rule.name.as_str(), name.as_str())
        });
        result.context("manifest has not been parsed")?
    })
}

#[then("the first target command is {command}")]
fn first_target_command(command: String) -> Result<()> {
    let command = CommandText::new(command);
    with_world(|world| {
        let result = world.manifest.with_ref(|m| {
            let target = m.targets.first().context("missing target 1")?;
            match &target.recipe {
                Recipe::Command { command: actual } => {
                    assert_field_eq("target 1", "command", actual, command.as_str())
                }
                other => bail!("Expected command recipe, got: {other:?}"),
            }
        });
        result.context("manifest has not been parsed")?
    })
}

#[then("the manifest has {count:usize} targets")]
fn manifest_has_targets(count: usize) -> Result<()> {
    with_world(|world| {
        let actual = world
            .manifest
            .with_ref(|m| m.targets.len())
            .context("manifest has not been parsed")?;
        ensure!(
            actual == count,
            "expected manifest to have {count} targets, got {actual}"
        );
        Ok(())
    })
}

#[then("the manifest has {count:usize} macros")]
fn manifest_has_macros(count: usize) -> Result<()> {
    with_world(|world| {
        let actual = world
            .manifest
            .with_ref(|m| m.macros.len())
            .context("manifest has not been parsed")?;
        ensure!(
            actual == count,
            "expected manifest to have {count} macros, got {actual}"
        );
        Ok(())
    })
}

#[then("the macro {index:usize} signature is {signature}")]
fn macro_signature_is(index: usize, signature: String) -> Result<()> {
    let signature = MacroSignature::new(signature);
    ensure!(index > 0, "macros use 1-based index");
    with_world(|world| {
        let result = world.manifest.with_ref(|m| {
            let macro_def = m
                .macros
                .get(index - 1)
                .with_context(|| format!("missing macro {index}"))?;
            assert_field_eq(
                &format!("macro {index}"),
                "signature",
                macro_def.signature.as_str(),
                signature.as_str(),
            )
        });
        result.context("manifest has not been parsed")?
    })
}

#[when("the manifest has targets named {names}")]
#[then("the manifest has targets named {names}")]
fn manifest_has_targets_named(names: String) -> Result<()> {
    let names = NamesList::new(names);
    let expected: BTreeSet<String> = names.to_set();
    with_world(|world| {
        let result = world.manifest.with_ref(|m| {
            let actual: BTreeSet<String> = m
                .targets
                .iter()
                .enumerate()
                .map(|(i, target)| {
                    get_string_from_string_or_list(&target.name, "name").with_context(|| {
                        format!(
                            "failed to extract name for target at index {} (raw: {:?})",
                            i + 1,
                            target
                        )
                    })
                })
                .collect::<Result<_>>()?;
            let missing: BTreeSet<_> = expected.difference(&actual).cloned().collect();
            let extra: BTreeSet<_> = actual.difference(&expected).cloned().collect();
            ensure!(
                missing.is_empty() && extra.is_empty(),
                "target names differ\nmissing: {missing:?}\nextra: {extra:?}"
            );
            Ok(())
        });
        result.context("manifest has not been parsed")?
    })
}

#[then("the target {index:usize} name is {name}")]
fn target_name_n(index: usize, name: String) -> Result<()> {
    let name = TargetName::new(name);
    ensure!(index > 0, "target index is 1-based");
    with_world(|world| {
        let result = world.manifest.with_ref(|m| {
            let target = m
                .targets
                .get(index - 1)
                .with_context(|| format!("missing target {index}"))?;
            let actual = get_string_from_string_or_list(&target.name, "name")?;
            ensure!(
                actual == name.as_str(),
                "expected target {index} name '{}', got '{actual}'",
                name
            );
            Ok(())
        });
        result.context("manifest has not been parsed")?
    })
}

#[then("the target {index:usize} command is {command}")]
fn target_command_n(index: usize, command: String) -> Result<()> {
    let command = CommandText::new(command);
    ensure!(index > 0, "target index is 1-based");
    with_world(|world| {
        let result = world.manifest.with_ref(|m| {
            let target = m
                .targets
                .get(index - 1)
                .with_context(|| format!("missing target {index}"))?;
            match &target.recipe {
                Recipe::Command { command: actual } => assert_field_eq(
                    &format!("target {index}"),
                    "command",
                    actual,
                    command.as_str(),
                ),
                other => bail!("Expected command recipe, got: {other:?}"),
            }
        });
        result.context("manifest has not been parsed")?
    })
}

#[then("the target {index:usize} index is {expected:usize}")]
fn target_index_n(index: usize, expected: usize) -> Result<()> {
    ensure!(index > 0, "target index is 1-based");
    with_world(|world| {
        let result = world.manifest.with_ref(|m| {
            let target = m
                .targets
                .get(index - 1)
                .with_context(|| format!("missing target {index}"))?;
            let index_value = target
                .vars
                .get(INDEX_KEY)
                .with_context(|| format!("target {index} missing '{INDEX_KEY}' variable"))?
                .as_u64()
                .with_context(|| format!("target {index} index is not an integer"))?;
            let actual = usize::try_from(index_value)
                .with_context(|| format!("target {index} index does not fit into usize"))?;
            ensure!(
                actual == expected,
                "unexpected index for target {index}: expected {expected}, got {actual}"
            );
            Ok(())
        });
        result.context("manifest has not been parsed")?
    })
}

#[then("the target {index:usize} has source {source}")]
fn target_has_source(index: usize, source: String) -> Result<()> {
    let source = SourcePath::new(source);
    ensure!(index > 0, "target index is 1-based");
    with_world(|world| {
        let result = world.manifest.with_ref(|m| {
            let target = m
                .targets
                .get(index - 1)
                .with_context(|| format!("missing target {index}"))?;
            assert_list_contains(&target.sources, source.as_str())
        });
        result.context("manifest has not been parsed")?
    })
}

#[then("the target {index:usize} has dep {dep}")]
fn target_has_dep(index: usize, dep: String) -> Result<()> {
    let dep = DepName::new(dep);
    ensure!(index > 0, "target index is 1-based");
    with_world(|world| {
        let result = world.manifest.with_ref(|m| {
            let target = m
                .targets
                .get(index - 1)
                .with_context(|| format!("missing target {index}"))?;
            assert_list_contains(&target.deps, dep.as_str())
        });
        result.context("manifest has not been parsed")?
    })
}

#[then("the target {index:usize} has order-only dep {dep}")]
fn target_has_order_only_dep(index: usize, dep: String) -> Result<()> {
    let dep = DepName::new(dep);
    ensure!(index > 0, "target index is 1-based");
    with_world(|world| {
        let result = world.manifest.with_ref(|m| {
            let target = m
                .targets
                .get(index - 1)
                .with_context(|| format!("missing target {index}"))?;
            assert_list_contains(&target.order_only_deps, dep.as_str())
        });
        result.context("manifest has not been parsed")?
    })
}

#[then("the target {index:usize} script is {script}")]
fn target_script_is(index: usize, script: String) -> Result<()> {
    let script = ScriptText::new(script);
    ensure!(index > 0, "target index is 1-based");
    with_world(|world| {
        let result = world.manifest.with_ref(|m| {
            let target = m
                .targets
                .get(index - 1)
                .with_context(|| format!("missing target {index}"))?;
            match &target.recipe {
                Recipe::Script { script: actual } => {
                    ensure!(
                        actual == script.as_str(),
                        "expected target {index} script '{}', got '{actual}'",
                        script
                    );
                    Ok(())
                }
                other => bail!("Expected script recipe, got: {other:?}"),
            }
        });
        result.context("manifest has not been parsed")?
    })
}

#[then("the target {index:usize} rule is {rule}")]
fn target_rule_is(index: usize, rule: String) -> Result<()> {
    let rule = RuleName::new(rule);
    ensure!(index > 0, "target index is 1-based");
    with_world(|world| {
        let result = world.manifest.with_ref(|m| {
            let target = m
                .targets
                .get(index - 1)
                .with_context(|| format!("missing target {index}"))?;
            match &target.recipe {
                Recipe::Rule { rule: actual } => {
                    let actual_str = get_string_from_string_or_list(actual, "rule")?;
                    ensure!(
                        actual_str == rule.as_str(),
                        "expected target {index} rule '{}', got '{actual_str}'",
                        rule
                    );
                    Ok(())
                }
                other => bail!("Expected rule recipe, got: {other:?}"),
            }
        });
        result.context("manifest has not been parsed")?
    })
}
