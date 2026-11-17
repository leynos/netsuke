//! Shape-validation tests for trusted `shells` declarations.
//!
//! These drive the real discovery pipeline, so each rejection proves that the
//! shape checks in `src/cli/discovery/shell_definitions.rs` are reachable from
//! operator configuration. Every case names the exact field key and the whole
//! rule text: removing or weakening one check, or dropping part of a message,
//! would fail here rather than pass unnoticed.
//!
//! The wrong-typed cases also plant a distinctive value in the malformed
//! definition and assert that neither it nor serde's `invalid type` wording
//! reaches the operator. A definition's values are operator-supplied text, so a
//! diagnostic that echoed one would break the redaction rule these checks exist
//! to keep.

use super::support::{
    Startup, ensure_rejected_before_compilation, generate_with_operator_config, stderr_of,
};
use anyhow::{Result, ensure};
use rstest::rstest;

/// Operator-supplied text a diagnostic must never render.
///
/// `wording` is what serde would say about the value and `value` is the
/// distinctive text planted in the malformed definition. Both must be absent:
/// the first catches a check that lets serde report the fault, and the second
/// catches a check that reports serde's message verbatim.
struct Echo<'a> {
    /// Serde's own wording for a wrong type.
    wording: &'a str,
    /// The rejected value, spelled to be unmistakable in the output.
    value: &'a str,
}

#[rstest]
#[case::unknown_field(
    "[[shells]]\nname = \"dash\"\nexecutable = \"dash\"\nargs = [\"-c\"]\nextra = \"x\"\n",
    "shells[0].extra",
    "is not a field of a shell definition; expected name, executable, and args",
    None
)]
#[case::misspelt_known_field(
    "[[shells]]\nname = \"dash\"\nexecutabl = \"dash\"\nargs = [\"-c\"]\n",
    "shells[0].executabl",
    "is not a field of a shell definition; expected name, executable, and args",
    None
)]
#[case::missing_name(
    "[[shells]]\nexecutable = \"dash\"\nargs = [\"-c\"]\n",
    "shells[0].name",
    "is required",
    None
)]
#[case::missing_executable(
    "[[shells]]\nname = \"dash\"\nargs = [\"-c\"]\n",
    "shells[0].executable",
    "is required",
    None
)]
#[case::missing_args(
    "[[shells]]\nname = \"dash\"\nexecutable = \"dash\"\n",
    "shells[0].args",
    "is required",
    None
)]
#[case::non_list_shells(
    "shells = \"dash\"\n",
    "shells",
    "must be a list of shell definitions",
    None
)]
#[case::non_table_entry(
    "shells = [\"dash\"]\n",
    "shells[0]",
    "must be a table declaring name, executable, and args",
    None
)]
#[case::wrong_typed_name(
    "[[shells]]\nname = 987654321\nexecutable = \"dash\"\nargs = [\"-c\"]\n",
    "shells[0].name",
    "must be a string",
    Some(Echo { wording: "invalid type", value: "987654321" })
)]
#[case::wrong_typed_executable(
    "[[shells]]\nname = \"dash\"\nexecutable = [\"rejected-executable\"]\nargs = [\"-c\"]\n",
    "shells[0].executable",
    "must be a string",
    Some(Echo { wording: "invalid type", value: "rejected-executable" })
)]
#[case::wrong_typed_args(
    "[[shells]]\nname = \"dash\"\nexecutable = \"dash\"\nargs = \"rejected-args-vector\"\n",
    "shells[0].args",
    "must be a list of strings",
    Some(Echo { wording: "invalid type", value: "rejected-args-vector" })
)]
fn a_malformed_definition_is_rejected_by_shape(
    #[case] definition: &str,
    #[case] key: &str,
    #[case] rule: &str,
    #[case] echo: Option<Echo<'_>>,
) -> Result<()> {
    let startup = Startup::new()?;

    let output = generate_with_operator_config(&startup, definition)?;

    ensure_rejected_before_compilation(&startup, &output, key)?;
    let stderr = stderr_of(&output);
    ensure!(
        stderr.contains(rule),
        "diagnostic should state {rule:?}, got: {stderr}"
    );
    if let Some(Echo { wording, value }) = echo {
        for unexpected in [wording, value] {
            ensure!(
                !stderr.contains(unexpected),
                "diagnostic should not render {unexpected:?}, but does: {stderr}"
            );
        }
    }
    Ok(())
}
