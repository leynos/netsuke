//! OBL-JOIN-BUDGET: `shell_join` bounds the encoded result it builds.
//!
//! Issue #970 reported that a sequence of references to one generated string
//! made the filter allocate an encoded copy of every member and then the joined
//! string. The ceiling is a property of the filter itself, not of the manifest
//! that fed it, so these cases drive the registered filter directly for both
//! dialects and include an iterable that never ends: an unbounded input must be
//! stopped by the budget rather than by exhausting memory.
//!
//! The fixed tables pin the ceiling arithmetic and are the readable statement of
//! the contract. The generated property at the end checks the same contract
//! over member counts, lengths, and contents drawn on and around the boundary,
//! where the pre-check and the post-encode check can disagree about a member.

use anyhow::{Result, ensure};
use minijinja::value::Value;
use proptest::prelude::*;
use proptest::test_runner::FileFailurePersistence;
use rstest::rstest;

use super::property_support::{ALPHABET, POWERSHELL, SH, quote_value, render_with};

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

/// Join `endless` and require that the ceiling, not the input, ended it.
///
/// The assertion that the call returns at all is part of the contract: an
/// endless iterable that the budget did not stop would hang the test rather
/// than fail it, so the enclosing test's completion carries meaning too.
///
/// # Errors
///
/// Returns an error when the join succeeds, or when its diagnostic does not
/// carry the stable ceiling code.
fn assert_endless_iterable_is_stopped(endless: &Value) -> Result<()> {
    let error = render_with("{{ value | shell_join }}", SH, endless)
        .expect_err("an endless iterable must reach the ceiling rather than run forever");
    ensure!(
        format!("{error:#}").contains(LIMIT_CODE),
        "the endless iterable should fail on the ceiling: {error:#}"
    );
    Ok(())
}

/// An endless iterable of oversized members is stopped by the ceiling.
///
/// The post-encode check alone would also stop this input, so what the test
/// asserts is that the ceiling ends the sequence rather than the input: the
/// test's own completion is part of the assertion. The pre-check's separate
/// guarantee — that the member is never encoded — is pinned by the unit suite.
#[test]
fn an_endless_iterable_of_large_members_is_stopped() -> Result<()> {
    assert_endless_iterable_is_stopped(&Value::make_iterable(|| {
        std::iter::repeat("a".repeat(400_000))
    }))
}

/// An endless iterable of small members is also stopped, so the ceiling bounds
/// the total rather than only one member.
#[test]
fn an_endless_iterable_of_small_members_is_stopped() -> Result<()> {
    assert_endless_iterable_is_stopped(&Value::make_iterable(|| std::iter::repeat("abcdefg")))
}

/// Half of the ceiling, where two members plus a separating space cross it.
///
/// `LIMIT` is a power of two, so the shift is exact. It is written as a shift
/// because the workspace lints deny integer division.
const HALF_LIMIT: usize = LIMIT >> 1;

/// A member length that mostly stays small but seeds the boundary.
///
/// A fixed mebibyte ceiling is only exercised by large members, so the seeded
/// arms sit on the half-ceiling — where two members plus the separating space
/// cross it — and on the ceiling itself. The small arm keeps most cases cheap.
fn budgeted_length() -> impl Strategy<Value = usize> {
    prop_oneof![
        8 => 0..24_usize,
        2 => (HALF_LIMIT - 1)..=(HALF_LIMIT + 1),
        1 => Just(LIMIT),
        1 => Just(LIMIT + 1),
    ]
}

/// One word of a generated length, using only characters a recipe may carry.
fn budgeted_member() -> impl Strategy<Value = String> {
    (budgeted_length(), prop::sample::select(ALPHABET))
        .prop_map(|(length, character)| std::iter::repeat_n(character, length).collect())
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 24,
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(
            "tests/shell_filter_property_tests.proptest-regressions",
        ))),
        ..ProptestConfig::default()
    })]

    /// Every successful join is the per-member encodings in order; a join is
    /// refused exactly when that reference encoding exceeds the ceiling, and
    /// the refusal names a member that exists.
    #[test]
    fn the_ceiling_bounds_every_join(
        dialect in prop_oneof![Just(SH), Just(POWERSHELL)],
        values in prop::collection::vec(budgeted_member(), 0..4),
    ) {
        let subject = Value::from_serialize(&values);
        let mut expected = Vec::with_capacity(values.len());
        for member in &values {
            expected.push(
                quote_value(member, dialect)
                    .map_err(|error| TestCaseError::fail(format!("{error:#}")))?,
            );
        }
        let reference = expected.join(" ");
        match render_with("{{ values | shell_join }}", dialect, &subject) {
            Ok(joined) => {
                prop_assert!(
                    reference.len() <= LIMIT,
                    "a join of {} bytes fits the ceiling and must render; it produced {} bytes",
                    reference.len(),
                    joined.len()
                );
                prop_assert!(
                    joined == reference,
                    "the {dialect} join should be the per-member encodings"
                );
            }
            Err(error) => {
                prop_assert!(
                    reference.len() > LIMIT,
                    "a join of {} bytes fits the ceiling and must not be refused: {error:#}",
                    reference.len()
                );
                let message = format!("{error:#}");
                prop_assert!(
                    message.contains(LIMIT_CODE),
                    "a refusal should carry the stable code: {message}"
                );
                prop_assert!(
                    (0..values.len()).any(|index| {
                        message.contains(&format!("item \u{2068}{index}\u{2069}"))
                    }),
                    "the refusal should name a member index within the input: {message}"
                );
            }
        }
    }
}
