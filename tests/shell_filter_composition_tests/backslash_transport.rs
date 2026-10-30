//! Bash-transport regression coverage for backslash-bearing script bytes.
//!
//! The `Bash` renderer wraps the POSIX script transport in
//! `bash.exe -e -c "…"`. This case decodes that argument and runs it under the
//! host's own `sh`, which is the closest executable check available on a Unix
//! host, and asserts the reported payload reaches the final interpreter
//! without becoming shell syntax.

use super::{
    command_binding, decode_bash_transport, generate, posix_shell, skip_without_posix_shell,
};
use anyhow::{Context, Result, ensure};
use netsuke::ninja_gen::RecipeShell;
use rstest::rstest;
use std::process::Command;

/// The reported payload: an octal escape followed by a redirect.
const BACKSLASH_PAYLOAD: &str = r"\047;>injected;#";

/// Verify the Bash wrapper preserves script bytes containing a backslash.
#[rstest]
fn bash_script_transport_preserves_backslash_bytes() -> Result<()> {
    let Some(interpreter) = posix_shell() else {
        skip_without_posix_shell();
        return Ok(());
    };
    let source = format!(
        "netsuke_version: \"1.0.0\"\ntargets:\n  - name: out\n    script: |\n      printf '%s' '{BACKSLASH_PAYLOAD}'\n"
    );
    let ninja = generate(&source, RecipeShell::Bash)?;
    // Ninja collapses `$$` before the shell sees the text. This binding has no
    // doubled dollars, but the step keeps the model faithful to execution.
    let script = decode_bash_transport(&command_binding(&ninja)?)?.replace("$$", "$");

    let workspace = tempfile::tempdir()?;
    let output = Command::new(interpreter.as_str())
        .arg("-c")
        .arg(&script)
        .current_dir(workspace.path())
        .output()
        .context("run the decoded Bash transport under the host sh")?;
    ensure!(
        output.status.success(),
        "host sh rejected the decoded transport: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    ensure!(
        output.stdout == BACKSLASH_PAYLOAD.as_bytes(),
        "the Bash transport changed the script bytes: {:?}",
        String::from_utf8_lossy(&output.stdout)
    );
    ensure!(
        !workspace.path().join("injected").exists(),
        "the Bash transport executed the injected redirect"
    );
    Ok(())
}
