//! OBL-JOIN-BUDGET: `shell_join` bounds the encoded result it builds.
//!
//! Issue #970 reported that a sequence of references to one generated string
//! made the filter allocate an encoded copy of every member and then the joined
//! string. The ceiling is a property of the filter itself, not of the manifest
//! that fed it, so these cases drive the registered filter directly for both
//! dialects and include an iterable that never ends: an unbounded input must be
//! stopped by the budget rather than by exhausting memory.

use anyhow::{Result, ensure};
use minijinja::value::Value;
use rstest::rstest;

use super::property_support::{POWERSHELL, SH, render_with};

/// The stable diagnostic code the ceiling error carries.
const LIMIT_CODE: &str = "[netsuke::jinja::shell::join_limit]";

/// The ceiling in bytes, restated so the suite does not read the implementation
/// it is meant to check.
const LIMIT: usize = 1024 * 1024;

/// Render a join of `count` references to one `length`-byte word.
fn join_repeated(dialect: &str, length: usize, count: usize) -> Result<String> {
    let values = vec!["a".repeat(length); count];
    render_with(
        "{{ values | shell_join }}",
        dialect,
        &Value::from_serialize(&values),
    )
}

/// A repeated reference cannot amplify past the ceiling in either dialect.
#[rstest]
#[case::sh(SH)]
#[case::power_shell(POWERSHELL)]
fn a_repeated_reference_cannot_amplify_past_the_ceiling(#[case] dialect: &str) -> Result<()> {
    let error = join_repeated(dialect, 400_000, 3)
        .expect_err("three 400 kB members exceed the one-mebibyte ceiling");
    let message = format!("{error:#}");
    ensure!(
        message.contains(LIMIT_CODE),
        "the {dialect} rejection should carry the stable diagnostic code: {message}"
    );
    ensure!(
        message.contains(&LIMIT.to_string()),
        "the {dialect} rejection should state the ceiling it enforced: {message}"
    );
    ensure!(
        !message.contains("aaaa"),
        "the {dialect} rejection must not echo the content it refused: {message}"
    );
    Ok(())
}

/// The control: the same shape inside the ceiling still renders.
///
/// `sh` emits each inert word unchanged, so the result is
/// `1000 + 1 + 1000 + 1 + 1000`. PowerShell encloses each word in quotes, which
/// adds two bytes per member. Pinning both keeps the ceiling from being
/// satisfied by breaking ordinary joining.
#[rstest]
#[case::sh(SH, 3002)]
#[case::power_shell(POWERSHELL, 3008)]
fn a_join_within_the_ceiling_still_renders(
    #[case] dialect: &str,
    #[case] expected: usize,
) -> Result<()> {
    let joined = join_repeated(dialect, 1000, 3)?;
    ensure!(
        joined.len() == expected,
        "the {dialect} control join should be {expected} bytes, got {}: {joined:?}",
        joined.len()
    );
    Ok(())
}

/// An endless iterable of oversized members is stopped by the ceiling.
///
/// Without the pre-check the filter would encode and retain one member after
/// another forever; the test's own completion is part of the assertion.
#[test]
fn an_endless_iterable_of_large_members_is_stopped() -> Result<()> {
    let endless = Value::make_iterable(|| std::iter::repeat("a".repeat(400_000)));
    let error = render_with("{{ value | shell_join }}", SH, &endless)
        .expect_err("an endless iterable must reach the ceiling rather than run forever");
    ensure!(
        format!("{error:#}").contains(LIMIT_CODE),
        "the endless iterable should fail on the ceiling: {error:#}"
    );
    Ok(())
}

/// An endless iterable of small members is also stopped, so the ceiling bounds
/// the total rather than only one member.
#[test]
fn an_endless_iterable_of_small_members_is_stopped() -> Result<()> {
    let endless = Value::make_iterable(|| std::iter::repeat("abcdefg"));
    let error = render_with("{{ value | shell_join }}", SH, &endless)
        .expect_err("an endless iterable of small members must reach the ceiling");
    ensure!(
        format!("{error:#}").contains(LIMIT_CODE),
        "the endless iterable should fail on the ceiling: {error:#}"
    );
    Ok(())
}
