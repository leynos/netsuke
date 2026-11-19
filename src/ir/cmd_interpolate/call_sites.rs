//! Bind each reachable interpolation rejection to the reason it reports.
//!
//! The tables in `rejection.rs` prove that every variant renders its own fixed
//! text; these cases prove the call sites choose the intended variant. Swapping
//! two reasons at their call sites would keep every table row passing while a
//! reader was told the wrong thing about the recipe.
//!
//! `RejectionReason::UnsupportedMarkerPosition` has no case here. Both arms
//! that report it fire only when an internal source position is out of range,
//! and the only callers loop while that position is in range, so no manifest
//! can reach it.
//!
//! The module is named for what it pins rather than for the module it
//! complements: `tests/workflow_contracts/rust_module_layout_test.py` rejects
//! sibling modules sharing a prefix unless the pair is recorded as a deliberate
//! exception, and this pair is ordinary name symmetry rather than a group.

use super::*;

use camino::Utf8PathBuf;
use rstest::rstest;

/// Select the traversal whose call site a case exercises.
#[derive(Clone, Copy, Debug)]
enum Lowering {
    /// The `command:` traversal.
    Command,
    /// The POSIX `script:` traversal.
    Script,
}

impl Lowering {
    /// Lower one template and return the rejection it must produce.
    ///
    /// The bindings carry a hostile two-sided path so a case cannot pass by
    /// quoting a marker successfully instead of rejecting it.
    fn reject(self, template: &str, shell: RecipeShell) -> IrGenError {
        let hostile = Utf8PathBuf::from("\"; touch injected; #");
        let bindings = CommandBindings::new(
            std::slice::from_ref(&hostile),
            std::slice::from_ref(&hostile),
            shell,
        );
        let result = match self {
            Self::Command => interpolate_command_with_bindings(template, &bindings),
            Self::Script => interpolate_script_with_bindings(template, &bindings),
        };
        result.expect_err("the case must be rejected")
    }
}

/// Read the bounded reason from an interpolation rejection.
fn rejection_reason(error: &IrGenError) -> &'static str {
    match error {
        IrGenError::InvalidCommand { reason, .. } => reason,
        other => panic!("expected an interpolation rejection, got {other:?}"),
    }
}

/// Each rejection path must report the reason its syntax calls for.
#[rstest]
#[case::protected_region_command(
    Lowering::Command,
    RecipeShell::Posix,
    format!("echo `cat {INS_TOKEN}`"),
    "recipe marker inside a protected shell region"
)]
#[case::protected_region_script(
    Lowering::Script,
    RecipeShell::Posix,
    format!("echo `cat {INS_TOKEN}`"),
    "recipe marker inside a protected shell region"
)]
#[case::escape_before_marker_command(
    Lowering::Command,
    RecipeShell::Posix,
    format!("echo \\{INS_TOKEN}"),
    "escape sequence before a recipe marker"
)]
#[case::escape_before_marker_script(
    Lowering::Script,
    RecipeShell::Posix,
    format!("echo \\{INS_TOKEN}"),
    "escape sequence before a recipe marker"
)]
#[case::here_string(
    Lowering::Command,
    RecipeShell::PowerShell,
    format!("@\"\n{INS_TOKEN}\n\"@"),
    "recipe marker inside a PowerShell here-string"
)]
#[case::raw_argument(
    Lowering::Command,
    RecipeShell::PowerShell,
    format!("cmd /c --% type {INS_TOKEN}"),
    "recipe marker inside raw native command arguments"
)]
#[case::unmatched_backtick(
    Lowering::Command,
    RecipeShell::Posix,
    String::from("echo 'a`b'"),
    "unmatched backtick"
)]
#[case::unparseable_command(
    Lowering::Command,
    RecipeShell::Posix,
    String::from("echo \"unterminated"),
    "unparseable shell syntax"
)]
fn every_rejection_path_reports_its_reason(
    #[case] lowering: Lowering,
    #[case] shell: RecipeShell,
    #[case] template: String,
    #[case] expected: &'static str,
) {
    let error = lowering.reject(&template, shell);
    assert_eq!(
        rejection_reason(&error),
        expected,
        "the call site must report {expected:?} for {template:?}"
    );
    let rendered = error.to_string();
    assert!(
        rendered.contains(expected),
        "the rendered message must name {expected:?}: {rendered}"
    );
    assert!(
        !rendered.contains(&template),
        "the rendered message must not quote the template: {rendered}"
    );
}
