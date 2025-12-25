//! Assertions shared by stdlib-related BDD steps, providing
//! reusable checks for rendered output, errors, and cache behaviour.

use crate::bdd::fixtures::{RefCellOptionExt, strip_quotes, with_world};
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

#[then("the stdlib output equals {expected}")]
pub(crate) fn assert_stdlib_output(expected: String) -> Result<()> {
    let expected = strip_quotes(&expected);
    with_world(|world| {
        let output = world
            .stdlib_output
            .get()
            .context("expected stdlib output to be available")?;
        ensure!(
            output == expected,
            "expected stdlib output '{expected}', got '{output}'"
        );
        Ok(())
    })
}

fn stdlib_root_and_output() -> Result<(camino::Utf8PathBuf, String)> {
    with_world(|world| {
        let root = world
            .stdlib_root
            .get()
            .context("expected stdlib workspace root")?;
        let output = stdlib_output()?;
        Ok((root, output))
    })
}

fn stdlib_output() -> Result<String> {
    with_world(|world| {
        let output = world.stdlib_output.get();
        let error = world.stdlib_error.get();
        stdlib_output_or_error(output.as_deref(), error.as_deref()).map(String::from)
    })
}

fn stdlib_output_path() -> Result<camino::Utf8PathBuf> {
    stdlib_output().map(|s| camino::Utf8PathBuf::from(s))
}

#[then("the stdlib error contains {expected}")]
pub(crate) fn assert_stdlib_error(expected: String) -> Result<()> {
    let expected = strip_quotes(&expected);
    with_world(|world| {
        let error = world.stdlib_error.get().context("expected stdlib error")?;
        ensure!(
            error.contains(expected),
            "error `{error}` should contain `{expected}`"
        );
        Ok(())
    })
}

#[then("the stdlib template is impure")]
pub(crate) fn assert_stdlib_impure() -> Result<()> {
    with_world(|world| {
        let is_impure = world
            .stdlib_state
            .with_ref(|s| s.is_impure())
            .context("stdlib state should be initialised")?;
        ensure!(is_impure, "expected template to be impure");
        Ok(())
    })
}

#[then("the stdlib template is pure")]
pub(crate) fn assert_stdlib_pure() -> Result<()> {
    with_world(|world| {
        let is_impure = world
            .stdlib_state
            .with_ref(|s| s.is_impure())
            .context("stdlib state should be initialised")?;
        ensure!(!is_impure, "expected template to remain pure");
        Ok(())
    })
}

#[then("the stdlib workspace contains the fetch cache for stdlib url")]
pub(crate) fn assert_fetch_cache_present() -> Result<()> {
    with_world(|world| {
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
        let dir = Dir::open_ambient_dir(&root, ambient_authority())
            .context("open stdlib workspace root")?;
        let relative_cache = cache_path
            .strip_prefix(&root)
            .with_context(|| format!("derive cache path relative to {root}"))?;
        ensure!(
            dir.try_exists(relative_cache)
                .with_context(|| format!("check fetch cache at {cache_path}"))?,
            "expected fetch cache at {cache_path}"
        );
        Ok(())
    })
}

#[then("the stdlib output equals the workspace root")]
pub(crate) fn assert_stdlib_output_is_root() -> Result<()> {
    let (root, output) = stdlib_root_and_output()?;
    ensure!(
        output == root.as_str(),
        "expected output to equal workspace root"
    );
    Ok(())
}

#[then("the stdlib output matches the workspace path {path}")]
pub(crate) fn assert_stdlib_output_is_workspace_path(path: String) -> Result<()> {
    let path = strip_quotes(&path);
    let (root, output) = stdlib_root_and_output()?;
    let expected = root.join(path);
    ensure!(
        output == expected.as_str(),
        "expected output '{}', got '{output}'",
        expected
    );
    Ok(())
}

#[then("the stdlib output is the workspace executable {path}")]
pub(crate) fn assert_stdlib_output_is_workspace_executable(path: String) -> Result<()> {
    let path = strip_quotes(&path);
    let relative = camino::Utf8PathBuf::from(path);
    let (root, output) = stdlib_root_and_output()?;
    let expected = resolve_executable_path(&root, relative.as_path());
    ensure!(
        output == expected.as_str(),
        "expected stdlib output '{expected}' but was '{output}'"
    );
    Ok(())
}

#[then("the stdlib output is an ISO8601 UTC timestamp")]
pub(crate) fn assert_stdlib_output_is_utc_timestamp() -> Result<()> {
    let output = stdlib_output()?;
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

#[then("the stdlib output offset is {expected}")]
pub(crate) fn assert_stdlib_output_offset(expected: String) -> Result<()> {
    let expected = strip_quotes(&expected);
    let output = stdlib_output()?;
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
pub(crate) fn assert_stdlib_output_file_min_size(minimum: u64) -> Result<()> {
    let path = stdlib_output_path()?;
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

#[then("the stdlib output file contains only {expected}")]
pub(crate) fn assert_stdlib_output_file_uniform(expected: String) -> Result<()> {
    let pattern = strip_quotes(&expected);
    ensure!(
        pattern.chars().count() == 1,
        "expected a single-character pattern but received '{pattern}'",
    );
    let bytes = pattern.as_bytes();
    ensure!(
        bytes.len() == 1,
        "expected a single-byte pattern but received '{pattern}'",
    );
    let Some(target) = bytes.first().copied() else {
        bail!("pattern should contain a single byte");
    };
    let path = stdlib_output_path()?;
    let data = fs::read(path.as_std_path())
        .with_context(|| format!("read stdlib output file {}", path.as_str()))?;
    ensure!(
        data.iter().all(|byte| *byte == target),
        "expected {} to contain only '{pattern}'",
        path,
    );
    Ok(())
}

#[then("the stdlib output file equals the stdlib text")]
pub(crate) fn assert_stdlib_output_equals_text() -> Result<()> {
    let expected = with_world(|world| {
        world
            .stdlib_text
            .get()
            .context("expected stdlib template text to be configured")
    })?;
    let path = stdlib_output_path()?;
    let data = fs::read_to_string(path.as_std_path())
        .with_context(|| format!("read stdlib output file {}", path.as_str()))?;
    ensure!(
        data == expected,
        "expected streamed output to match configured text"
    );
    Ok(())
}
