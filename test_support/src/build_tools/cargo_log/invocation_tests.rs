//! Unit tests for the reading of a logged `cargo` record.
//!
//! A sibling file rather than an inline module, so neither it nor the
//! invocation reader it exercises exceeds the 400-line module limit.

use anyhow::{Context, Result, ensure};
use rstest::rstest;

use super::CargoInvocation;
use crate::build_tools::cargo_log::RECORD_SEPARATOR;

/// Build a record the fake `cargo` could have written, varying only
/// `RUSTFLAGS`.
///
/// Every other field is present because [`CargoInvocation::parse`] rejects a
/// record with one missing, which is what keeps a harness bug from reading as
/// an observation. Fallible rather than panicking: arranging a fixture can
/// fail, and only a test body may treat a failure as the verdict.
fn recorded(rustflags: &str) -> Result<CargoInvocation> {
    let record = format!(
        "{RECORD_SEPARATOR}\n\
         arguments\tclippy\n\
         toolchain\tnightly\n\
         path\t/usr/bin\n\
         rustflags\t{rustflags}\n\
         rustflags_set\tyes\n\
         wrapper\t\n\
         wrapper_set\t\n\
         workspace_wrapper\t\n\
         workspace_wrapper_set\t\n\
         encoded_rustflags\t\n\
         encoded_rustflags_set\t\n\
         build_dir\t\n\
         target_dir\t\n\
         target_state\tabsent\n\
         touch_mtime\t\n"
    );
    let body = record
        .split(RECORD_SEPARATOR)
        .nth(1)
        .context("the record follows its separator")?;
    CargoInvocation::parse(body)
}

/// An option and its value must be matched where they actually sit.
///
/// The discriminating case is `-D dead_code warnings`: both tokens are
/// present, so a membership check reports the warning policy as enforced,
/// while the `-D` applies to `dead_code` and `warnings` is a stray token
/// denying nothing. A gate contract written on membership therefore passes for
/// a recipe that dropped `-D warnings` entirely.
#[rstest]
#[case::exact("-D warnings", true)]
#[case::surrounded("-Zthreads=8 -D warnings -Cdebuginfo=0", true)]
#[case::split_by_another_lint("-D dead_code warnings", false)]
#[case::value_only("warnings", false)]
#[case::option_only("-D", false)]
fn a_grouped_flag_is_matched_as_a_contiguous_sequence(
    #[case] rustflags: &str,
    #[case] expected: bool,
) -> Result<()> {
    let invocation = recorded(rustflags)?;
    let found = invocation.rustflags_contain_sequence(&["-D", "warnings"]);
    ensure!(
        found == expected,
        "`{rustflags}` should {}deny warnings",
        if expected { "" } else { "not " }
    );
    Ok(())
}

/// The membership check is the one that cannot tell those apart, which is why
/// the grouped assertions no longer use it.
///
/// Stated rather than assumed: were this to stop holding, the sequence helper
/// would be guarding against nothing and the two could be merged.
#[test]
fn the_membership_check_accepts_the_split_case() -> Result<()> {
    let invocation = recorded("-D dead_code warnings")?;
    ensure!(
        invocation.rustflags_contain(&["-D", "warnings"]),
        "membership sees both tokens regardless of where they sit"
    );
    Ok(())
}

/// An empty sequence matches nothing, rather than vacuously matching
/// everything. A caller that computed its flags and got none would otherwise
/// assert success over an empty claim.
#[test]
fn an_empty_sequence_never_matches() -> Result<()> {
    let invocation = recorded("-D warnings")?;
    ensure!(
        !invocation.rustflags_contain_sequence(&[]),
        "an empty sequence is not a claim that can hold"
    );
    Ok(())
}
