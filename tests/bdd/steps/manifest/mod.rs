//! Step definitions for manifest parsing scenarios.
//!
//! This module is split into:
//! - `mod.rs` - Shared helpers and parsing/validation steps
//! - `targets.rs` - Target-specific assertion steps

mod targets;

use crate::bdd::fixtures::{RefCellOptionExt, TestWorld};
use crate::bdd::helpers::parse_store::store_parse_outcome;
use crate::bdd::types::{
    EnvVarKey, EnvVarValue, ErrorPattern, ManifestPath, RuleName, VersionString,
};
use anyhow::{Context, Result, bail, ensure};
use netsuke::{ast::StringOrList, manifest};
use rstest_bdd_macros::{given, then, when};
use std::ffi::OsStr;
use test_support::display_error_chain;
use test_support::env::{remove_var, set_var};

// ---------------------------------------------------------------------------
// Helper functions (shared with targets.rs)
// ---------------------------------------------------------------------------

/// Enhance an error with manifest parse error context if available.
///
/// When manifest parsing fails, the error is stored in `manifest_error` but
/// not propagated. This helper retrieves any stored error and includes it
/// in the error context, making diagnosis easier.
pub(super) fn with_manifest_error_context<T>(world: &TestWorld, result: Result<T>) -> Result<T> {
    if result.is_ok() {
        return result;
    }
    if let Some(parse_err) = world.manifest_error.get() {
        result.with_context(|| format!("manifest parse error: {parse_err}"))
    } else {
        result
    }
}

pub(super) fn get_string_from_string_or_list(
    value: &StringOrList,
    field_name: &str,
) -> Result<String> {
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

fn parse_manifest_inner(world: &TestWorld, path: &ManifestPath) {
    let outcome = manifest::from_path(path.as_str()).map_err(|e| display_error_chain(e.as_ref()));
    store_parse_outcome(&world.manifest, &world.manifest_error, outcome);
}

fn assert_manifest(world: &TestWorld) -> Result<()> {
    ensure!(
        world.manifest.is_some(),
        "manifest should have been parsed successfully"
    );
    Ok(())
}

fn assert_parsed(world: &TestWorld) -> Result<()> {
    ensure!(
        world.manifest.is_some() || world.manifest_error.is_filled(),
        "manifest should have been parsed"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Environment variable helpers
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Given steps
// ---------------------------------------------------------------------------

#[given("the environment variable {key:string} is set to {value:string}")]
fn set_env_var_step(world: &TestWorld, key: &str, value: &str) -> Result<()> {
    let key = EnvVarKey::new(key);
    let value = EnvVarValue::new(value);
    ensure!(
        !key.as_str().is_empty(),
        "environment variable name must not be empty"
    );
    let expanded = expand_env(value.as_str());
    let previous = set_var(key.as_str(), OsStr::new(&expanded));
    world.track_env_var(key.into_string(), previous);
    Ok(())
}

#[given("the environment variable {key:string} is unset")]
fn unset_env_var_step(world: &TestWorld, key: &str) -> Result<()> {
    let key = EnvVarKey::new(key);
    ensure!(
        !key.as_str().is_empty(),
        "environment variable name must not be empty"
    );
    let previous = remove_var(key.as_str());
    world.track_env_var(key.into_string(), previous);
    Ok(())
}

#[given("the manifest file {path:string} is parsed")]
#[when("the manifest file {path:string} is parsed")]
fn parse_manifest(world: &TestWorld, path: &str) -> Result<()> {
    let path = ManifestPath::new(path);
    ensure!(
        !path.as_str().trim().is_empty(),
        "manifest path must not be an empty string"
    );
    parse_manifest_inner(world, &path);
    Ok(())
}

// ---------------------------------------------------------------------------
// When steps
// ---------------------------------------------------------------------------

#[when("the parsing result is checked")]
fn when_parsing_result_checked(world: &TestWorld) -> Result<()> {
    assert_parsed(world)
}

#[when("the manifest is checked")]
#[when("the version is checked")]
#[when("the flags are checked")]
#[when("the rules are checked")]
fn when_manifest_aspects_checked(world: &TestWorld) -> Result<()> {
    assert_manifest(world)
}

// ---------------------------------------------------------------------------
// Then steps - manifest-level assertions
// ---------------------------------------------------------------------------

#[then("the manifest version is {version:string}")]
fn manifest_version(world: &TestWorld, version: &str) -> Result<()> {
    let version = VersionString::new(version);
    let actual = world
        .manifest
        .with_ref(|m| m.netsuke_version.to_string())
        .context("manifest has not been parsed");
    let actual = with_manifest_error_context(world, actual)?;
    ensure!(
        actual == version.as_str(),
        "expected manifest version '{version}', got '{actual}'"
    );
    Ok(())
}

#[then("parsing the manifest fails")]
fn manifest_parse_error(world: &TestWorld) -> Result<()> {
    ensure!(
        world.manifest_error.is_filled(),
        "expected manifest parsing to record an error"
    );
    Ok(())
}

#[then("the error message contains {text:string}")]
fn manifest_error_contains(world: &TestWorld, text: &str) -> Result<()> {
    let text = ErrorPattern::new(text);
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
}

#[then("the first rule name is {name:string}")]
fn first_rule_name(world: &TestWorld, name: &str) -> Result<()> {
    let name = RuleName::new(name);
    let result = world.manifest.with_ref(|m| {
        let rule = m
            .rules
            .first()
            .context("manifest does not contain any rules")?;
        ensure!(
            rule.name == name.as_str(),
            "expected first rule name '{}', got '{}'",
            name,
            rule.name
        );
        Ok(())
    });
    with_manifest_error_context(world, result.context("manifest has not been parsed"))?
}

#[then("the first action is phony")]
fn first_action_phony(world: &TestWorld) -> Result<()> {
    let result = world.manifest.with_ref(|m| {
        let first = m
            .actions
            .first()
            .context("manifest does not contain any actions")?;
        ensure!(first.phony, "expected first action to be marked phony");
        Ok(())
    });
    with_manifest_error_context(world, result.context("manifest has not been parsed"))?
}
