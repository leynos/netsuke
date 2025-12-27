//! Assertions shared by stdlib-related BDD steps, providing
//! reusable checks for rendered output, errors, and cache behaviour.

use crate::bdd::fixtures::{RefCellOptionExt, TestWorld};
use anyhow::{Context, Result, bail, ensure};
use cap_std::{ambient_authority, fs_utf8::Dir};
use rstest_bdd_macros::then;
use std::fs;
use test_support::hash;
use test_support::stdlib_assert::stdlib_output_or_error;
use time::{Duration, OffsetDateTime, UtcOffset};
use url::Url;

use super::parsing::{parse_expected_offset, parse_iso_timestamp};
use super::workspace::resolve_executable_path;

#[then("the stdlib output equals {expected:string}")]
pub(crate) fn assert_stdlib_output(world: &TestWorld, expected: &str) -> Result<()> {
    let output = world
        .stdlib_output
        .get()
        .context("expected stdlib output to be available")?;
    ensure!(
        output == expected,
        "expected stdlib output '{expected}', got '{output}'"
    );
    Ok(())
}

fn stdlib_root_and_output(world: &TestWorld) -> Result<(camino::Utf8PathBuf, String)> {
    let root = world
        .stdlib_root
        .get()
        .context("expected stdlib workspace root")?;
    let output = stdlib_output(world)?;
    Ok((root, output))
}

fn stdlib_output(world: &TestWorld) -> Result<String> {
    let output = world.stdlib_output.get();
    let error = world.stdlib_error.get();
    stdlib_output_or_error(output.as_deref(), error.as_deref()).map(String::from)
}

fn stdlib_output_path(world: &TestWorld) -> Result<camino::Utf8PathBuf> {
    stdlib_output(world).map(camino::Utf8PathBuf::from)
}

/// Check that the stdlib template purity state matches the expected value.
fn assert_purity_state(world: &TestWorld, expected_impure: bool) -> Result<()> {
    let is_impure = world
        .stdlib_state
        .with_ref(|s| s.is_impure())
        .context("stdlib state should be initialised")?;
    let state_name = if expected_impure { "impure" } else { "pure" };
    ensure!(
        is_impure == expected_impure,
        "expected template to be {state_name}"
    );
    Ok(())
}

#[then("the stdlib error contains {expected:string}")]
pub(crate) fn assert_stdlib_error(world: &TestWorld, expected: &str) -> Result<()> {
    let error = world.stdlib_error.get().context("expected stdlib error")?;
    ensure!(
        error.contains(expected),
        "error `{error}` should contain `{expected}`"
    );
    Ok(())
}

#[then("the stdlib template is impure")]
pub(crate) fn assert_stdlib_impure(world: &TestWorld) -> Result<()> {
    assert_purity_state(world, true)
}

#[then("the stdlib template is pure")]
pub(crate) fn assert_stdlib_pure(world: &TestWorld) -> Result<()> {
    assert_purity_state(world, false)
}

#[then("the stdlib workspace contains the fetch cache for stdlib url")]
pub(crate) fn assert_fetch_cache_present(world: &TestWorld) -> Result<()> {
    let root = world
        .stdlib_root
        .get()
        .context("expected stdlib workspace root")?;
    let url = world
        .stdlib_url
        .get()
        .context("expected stdlib url for cache check")?;
    let parsed_url = Url::parse(&url).context("canonicalise stdlib cache URL")?;
    let key = hash::sha256_hex(parsed_url.as_str().as_bytes());
    let cache_path = camino::Utf8PathBuf::from(&root)
        .join(".netsuke")
        .join("fetch")
        .join(key);
    let dir =
        Dir::open_ambient_dir(&root, ambient_authority()).context("open stdlib workspace root")?;
    let relative_cache = cache_path
        .strip_prefix(&root)
        .with_context(|| format!("derive cache path relative to {root}"))?;
    ensure!(
        dir.try_exists(relative_cache)
            .with_context(|| format!("check fetch cache at {cache_path}"))?,
        "expected fetch cache at {cache_path}"
    );
    Ok(())
}

#[then("the stdlib output equals the workspace root")]
pub(crate) fn assert_stdlib_output_is_root(world: &TestWorld) -> Result<()> {
    let (root, output) = stdlib_root_and_output(world)?;
    ensure!(
        output == root.as_str(),
        "expected output to equal workspace root"
    );
    Ok(())
}

#[then("the stdlib output matches the workspace path {path:string}")]
pub(crate) fn assert_stdlib_output_is_workspace_path(world: &TestWorld, path: &str) -> Result<()> {
    let (root, output) = stdlib_root_and_output(world)?;
    let expected = root.join(path);
    ensure!(
        output == expected.as_str(),
        "expected output '{}', got '{output}'",
        expected
    );
    Ok(())
}

#[then("the stdlib output is the workspace executable {path:string}")]
pub(crate) fn assert_stdlib_output_is_workspace_executable(
    world: &TestWorld,
    path: &str,
) -> Result<()> {
    let relative = camino::Utf8PathBuf::from(path);
    let (root, output) = stdlib_root_and_output(world)?;
    let expected = resolve_executable_path(&root, relative.as_path());
    ensure!(
        output == expected.as_str(),
        "expected stdlib output '{expected}' but was '{output}'"
    );
    Ok(())
}

#[then("the stdlib output is an ISO8601 UTC timestamp")]
pub(crate) fn assert_stdlib_output_is_utc_timestamp(world: &TestWorld) -> Result<()> {
    let output = stdlib_output(world)?;
    let parsed = parse_iso_timestamp(&output)?;
    let now = OffsetDateTime::now_utc();
    let delta = (now - parsed).abs();
    ensure!(
        delta <= Duration::seconds(5),
        "timestamp `{output}` should be within five seconds of now",
    );
    ensure!(
        parsed.offset() == UtcOffset::UTC,
        "timestamp `{output}` should have a UTC offset"
    );
    Ok(())
}

#[then("the stdlib output offset is {expected:string}")]
pub(crate) fn assert_stdlib_output_offset(world: &TestWorld, expected: &str) -> Result<()> {
    let output = stdlib_output(world)?;
    let parsed = parse_iso_timestamp(&output)?;
    let expected_offset = parse_expected_offset(expected)?;
    ensure!(
        parsed.offset() == expected_offset,
        "timestamp `{output}` offset {:?} did not match expected {expected}",
        parsed.offset()
    );
    Ok(())
}

#[then("the stdlib output file has at least {minimum:u64} bytes")]
pub(crate) fn assert_stdlib_output_file_min_size(world: &TestWorld, minimum: u64) -> Result<()> {
    let path = stdlib_output_path(world)?;
    let metadata = fs::metadata(path.as_std_path())
        .with_context(|| format!("stat stdlib output file {}", path.as_str()))?;
    ensure!(
        metadata.len() >= minimum,
        "expected {} to contain at least {minimum} bytes but found {}",
        path,
        metadata.len(),
    );
    Ok(())
}

#[then("the stdlib output file contains only {expected:string}")]
pub(crate) fn assert_stdlib_output_file_uniform(world: &TestWorld, expected: &str) -> Result<()> {
    ensure!(
        expected.chars().count() == 1,
        "expected a single-character pattern but received '{expected}'",
    );
    let bytes = expected.as_bytes();
    ensure!(
        bytes.len() == 1,
        "expected a single-byte pattern but received '{expected}'",
    );
    let Some(target) = bytes.first().copied() else {
        bail!("pattern should contain a single byte");
    };
    let path = stdlib_output_path(world)?;
    let data = fs::read(path.as_std_path())
        .with_context(|| format!("read stdlib output file {}", path.as_str()))?;
    ensure!(
        data.iter().all(|byte| *byte == target),
        "expected {} to contain only '{expected}'",
        path,
    );
    Ok(())
}

#[then("the stdlib output file equals the stdlib text")]
pub(crate) fn assert_stdlib_output_equals_text(world: &TestWorld) -> Result<()> {
    let expected = world
        .stdlib_text
        .get()
        .context("expected stdlib template text to be configured")?;
    let path = stdlib_output_path(world)?;
    let data = fs::read_to_string(path.as_std_path())
        .with_context(|| format!("read stdlib output file {}", path.as_str()))?;
    ensure!(
        data == expected,
        "expected streamed output to match configured text"
    );
    Ok(())
}
