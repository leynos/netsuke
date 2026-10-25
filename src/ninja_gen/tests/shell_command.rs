//! Tests for shell command text accepted by Ninja generation.

use super::NamedAction;

/// Retain support for shell command text with nested quoting and subshells.
#[test]
fn assert_shell_command_tolerates_complex_syntax() {
    let command = r#"/bin/sh -c "echo 'nested quotes' && echo \"double\" && (echo subshell)""#;
    NamedAction::assert_shell_command(command);
}
