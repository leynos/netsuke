//! Shape-validation tests for trusted `shells` declarations.
//!
//! These drive the real discovery pipeline, so each rejection proves that the
//! shape checks in `src/cli/discovery/shell_definitions.rs` are reachable from
//! operator configuration. Every case names the exact field key and rule text:
//! removing or weakening one check would accept the malformed definition here,
//! which is what makes the failure specific rather than incidental.
//!
//! The wrong-typed cases also assert that the rejected value is absent from the
//! message. A serde-generated message would quote it, and a definition's values
//! are operator-supplied text that a diagnostic must not print.

use super::support::{
    Startup, ensure_rejected_before_compilation, generate_with_operator_config, stderr_of,
};
use anyhow::{Result, ensure};
use rstest::rstest;

#[rstest]
#[case::unknown_field(
    "[[shells]]\nname = \"dash\"\nexecutable = \"dash\"\nargs = [\"-c\"]\nextra = \"x\"\n",
    "shells[0].extra",
    "is not a field of a shell definition",
    None
)]
#[case::misspelt_known_field(
    "[[shells]]\nname = \"dash\"\nexecutabl = \"dash\"\nargs = [\"-c\"]\n",
    "shells[0].executabl",
    "is not a field of a shell definition",
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
    "[[shells]]\nname = 1\nexecutable = \"dash\"\nargs = [\"-c\"]\n",
    "shells[0].name",
    "must be a string",
    Some("invalid type")
)]
#[case::wrong_typed_executable(
    "[[shells]]\nname = \"dash\"\nexecutable = [\"dash\"]\nargs = [\"-c\"]\n",
    "shells[0].executable",
    "must be a string",
    Some("invalid type")
)]
#[case::wrong_typed_args(
    "[[shells]]\nname = \"dash\"\nexecutable = \"dash\"\nargs = \"-c\"\n",
    "shells[0].args",
    "must be a list of strings",
    Some("invalid type")
)]
fn a_malformed_definition_is_rejected_by_shape(
    #[case] definition: &str,
    #[case] key: &str,
    #[case] rule: &str,
    #[case] unechoed: Option<&str>,
) -> Result<()> {
    let startup = Startup::new()?;

    let output = generate_with_operator_config(&startup, definition)?;

    ensure_rejected_before_compilation(&startup, &output, key)?;
    let stderr = stderr_of(&output);
    ensure!(
        stderr.contains(rule),
        "diagnostic should state {rule:?}, got: {stderr}"
    );
    if let Some(unexpected) = unechoed {
        ensure!(
            !stderr.contains(unexpected),
            "diagnostic should not render the rejected value, but contains {unexpected:?}: {stderr}"
        );
    }
    Ok(())
}
