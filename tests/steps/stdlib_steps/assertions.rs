//! Assertions shared by stdlib-related Cucumber steps, providing
//! reusable checks for rendered output, errors, and cache behaviour.
use crate::CliWorld;
use anyhow::{Context, Result, ensure};
use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir};
use cucumber::then;
use test_support::hash;
use time::{Duration, OffsetDateTime, UtcOffset};
use url::Url;

use super::parsing::{parse_expected_offset, parse_iso_timestamp};
use super::types::{ExpectedFragment, ExpectedOffset, ExpectedOutput, RelativePath};

#[then(regex = r#"^the stdlib output is "(.+)"$"#)]
pub(crate) fn assert_stdlib_output(
    world: &mut CliWorld,
    expected_output: ExpectedOutput,
) -> Result<()> {
    let expected_output = expected_output.into_inner();
    let output = world
        .stdlib_output
        .as_ref()
        .context("expected stdlib output to be available")?;
    ensure!(
        output == expected_output.as_str(),
        "expected stdlib output '{expected}', got '{output}'",
        expected = expected_output
    );
    Ok(())
}

fn stdlib_root_and_output(world: &CliWorld) -> Result<(&Utf8Path, &str)> {
    let root = world
        .stdlib_root
        .as_deref()
        .context("expected stdlib workspace root")?;
    let output = world
        .stdlib_output
        .as_deref()
        .context("expected stdlib output")?;
    Ok((root, output))
}

fn stdlib_output(world: &CliWorld) -> Result<&str> {
    world
        .stdlib_output
        .as_deref()
        .context("expected stdlib output")
}

#[then(regex = r#"^the stdlib error contains "(.+)"$"#)]
pub(crate) fn assert_stdlib_error(
    world: &mut CliWorld,
    expected_fragment: ExpectedFragment,
) -> Result<()> {
    let expected_fragment = expected_fragment.into_inner();
    let error = world
        .stdlib_error
        .as_ref()
        .context("expected stdlib error")?;
    ensure!(
        error.contains(expected_fragment.as_str()),
        "error `{error}` should contain `{expected}`",
        expected = expected_fragment
    );
    Ok(())
}

#[then("the stdlib template is impure")]
pub(crate) fn assert_stdlib_impure(world: &mut CliWorld) -> Result<()> {
    let state = world
        .stdlib_state
        .as_ref()
        .context("stdlib state should be initialised")?;
    ensure!(state.is_impure(), "expected template to be impure");
    Ok(())
}

#[then("the stdlib template is pure")]
pub(crate) fn assert_stdlib_pure(world: &mut CliWorld) -> Result<()> {
    let state = world
        .stdlib_state
        .as_ref()
        .context("stdlib state should be initialised")?;
    ensure!(!state.is_impure(), "expected template to remain pure");
    Ok(())
}

#[then("the stdlib workspace contains the fetch cache for stdlib url")]
pub(crate) fn assert_fetch_cache_present(world: &mut CliWorld) -> Result<()> {
    let root = world
        .stdlib_root
        .as_deref()
        .context("expected stdlib workspace root")?;
    let url = world
        .stdlib_url
        .as_ref()
        .context("expected stdlib url for cache check")?;
    let parsed_url = Url::parse(url).context("canonicalise stdlib cache URL")?;
    let key = hash::sha256_hex(parsed_url.as_str().as_bytes());
    let cache_path = root.join(".netsuke").join("fetch").join(key);
    let dir =
        Dir::open_ambient_dir(root, ambient_authority()).context("open stdlib workspace root")?;
    let relative_cache = cache_path
        .strip_prefix(root)
        .with_context(|| format!("derive cache path relative to {root}"))?;
    ensure!(
        dir.try_exists(relative_cache)
            .with_context(|| format!("check fetch cache at {cache_path}"))?,
        "expected fetch cache at {cache_path}"
    );
    Ok(())
}

#[then("the stdlib output equals the workspace root")]
pub(crate) fn assert_stdlib_output_is_root(world: &mut CliWorld) -> Result<()> {
    let (root, output) = stdlib_root_and_output(world)?;
    ensure!(
        output == root.as_str(),
        "expected output to equal workspace root"
    );
    Ok(())
}

#[then(regex = r#"^the stdlib output is the workspace path "(.+)"$"#)]
pub(crate) fn assert_stdlib_output_is_workspace_path(
    world: &mut CliWorld,
    relative_path: RelativePath,
) -> Result<()> {
    let (root, output) = stdlib_root_and_output(world)?;
    let expected = root.join(relative_path.into_path_buf());
    ensure!(
        output == expected.as_str(),
        "expected output '{}', got '{output}'",
        expected
    );
    Ok(())
}

#[then("the stdlib output is an ISO8601 UTC timestamp")]
pub(crate) fn assert_stdlib_output_is_utc_timestamp(world: &mut CliWorld) -> Result<()> {
    let output = stdlib_output(world)?;
    let parsed = parse_iso_timestamp(output)?;
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

#[then(regex = r#"^the stdlib output offset is "(.+)"$"#)]
pub(crate) fn assert_stdlib_output_offset(
    world: &mut CliWorld,
    expected_offset_text: ExpectedOffset,
) -> Result<()> {
    let expected_offset_text = expected_offset_text.into_inner();
    let output = stdlib_output(world)?;
    let parsed = parse_iso_timestamp(output)?;
    let expected_offset = parse_expected_offset(&expected_offset_text)?;
    ensure!(
        parsed.offset() == expected_offset,
        "timestamp `{output}` offset {:?} did not match expected {expected}",
        parsed.offset(),
        expected = expected_offset_text
    );
    Ok(())
}
