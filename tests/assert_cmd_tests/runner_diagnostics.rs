//! Verify human runner diagnostics through the real CLI process.

use super::create_netsuke_command;
use anyhow::{Context, Result, ensure};
use tempfile::tempdir;

/// Remove bidi isolates and terminal wrapping from localized output.
fn flatten_localized_text(rendered: &str) -> String {
    let without_bidi_isolates = rendered
        .chars()
        .filter(|character| !matches!(*character, '\u{2068}' | '\u{2069}'))
        .collect::<String>();
    without_bidi_isolates
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Render the localized missing-manifest diagnostic, code, and help on stderr.
#[test]
fn missing_manifest_renders_runner_diagnostic_and_help() -> Result<()> {
    let temp = tempdir().context("create an empty workspace")?;
    let output = create_netsuke_command(temp.path())
        .env("NO_COLOR", "1")
        .args(["--locale", "en-GB", "generate"])
        .output()
        .context("run generate without a Netsukefile")?;
    ensure!(
        !output.status.success(),
        "generate must fail when the workspace has no Netsukefile"
    );
    let stderr = String::from_utf8(output.stderr).context("diagnostic stderr is UTF-8")?;
    let flattened = flatten_localized_text(&stderr);
    ensure!(
        flattened.contains("netsuke::runner::manifest_not_found"),
        "stderr must carry the runner's stable diagnostic code: {stderr}"
    );
    ensure!(
        flattened.contains("Manifest 'Netsukefile' not found in the current directory."),
        "stderr must carry the localized summary: {stderr}"
    );
    ensure!(
        flattened.contains("Ensure the manifest exists or pass `--file` with the correct path."),
        "stderr must carry the recovery help: {stderr}"
    );
    Ok(())
}
