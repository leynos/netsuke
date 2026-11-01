//! Executable-shell regressions for backslashes that precede path markers.

use super::*;

use netsuke::ir::IrGenError;
use test_support::manifest::manifest_yaml;
#[cfg(unix)]
use test_support::ninja::ninja_integration_workspace;

/// The quote-leading path used by both the source list and the output name.
const INJECTING_PATH: &str = "\"; touch injected; #";

/// The recipe command produced by manifest rendering before interpolation.
const REJECTED_TEMPLATE: &str = "printf %s \\__NETSUKE_OUTS_PLACEHOLDER__";

/// Lower a manifest body for POSIX recipes and report the generated graph.
fn lower_posix(body: &str) -> Result<BuildGraph> {
    let yaml = manifest_yaml(body);
    let manifest = manifest::from_str(&yaml)?;
    Ok(BuildGraph::from_manifest_for_shell(
        &manifest,
        RecipeShell::Posix,
    )?)
}

/// Report the rejected command carried by an invalid-command error.
///
/// Lowering is reported through the test's opaque error type, so the
/// interpolation error is recovered before its command is inspected.
fn rejected_command(error: &anyhow::Error) -> Option<&str> {
    match error.downcast_ref::<IrGenError>()? {
        IrGenError::InvalidCommand { command, .. } => Some(command.trim_end()),
        _ => None,
    }
}

/// Build a single-target manifest around one recipe declaration.
///
/// The target names the quote-leading path and takes no sources, so the path
/// reaches the recipe only through the output marker and the target cannot
/// depend on itself.
fn target_manifest(recipe: &str, extra: &str) -> String {
    format!("targets:\n  - name: '{INJECTING_PATH}'\n{recipe}{extra}")
}

/// Reject a marker lowered directly after an unpaired backslash.
///
/// The trigger covers a quote-leading source path and output name across a
/// scalar command, a named rule's command list, and a script.
#[rstest]
#[case::scalar_command("    command: 'printf %s \\{{ outs }}'\n", "")]
#[case::rule_command_list(
    "    rule: build\n",
    "rules:\n  - name: build\n    command:\n      - 'printf %s \\{{ outs }}'\n"
)]
#[case::script("    script: |\n      printf %s \\{{ outs }}\n", "")]
fn markers_after_an_active_backslash_are_rejected(#[case] recipe: &str, #[case] extra: &str) {
    let error =
        lower_posix(&target_manifest(recipe, extra)).expect_err("lowering must reject the marker");
    assert_eq!(
        rejected_command(&error),
        Some(REJECTED_TEMPLATE),
        "unexpected error: {error:?}"
    );
}

/// Keep a paired backslash and its quote-leading path as one shell word.
///
/// The two backslashes are consumed as one escaped backslash before the marker
/// is reached, so the ordinary unquoted encoding applies and the path text
/// keeps its literal backslash prefix instead of becoming shell syntax.
///
/// Script recipes are covered at the interpolation layer instead: Ninja wraps
/// a script in a `printf %b` pipeline, and that wrapper already collapses a
/// backslash run before the script runs, independently of this contract.
#[cfg(unix)]
#[rstest]
#[case::scalar_command("    command: 'printf %s \\\\{{ outs }}'\n", "")]
#[case::rule_command_list(
    "    rule: build\n",
    "rules:\n  - name: build\n    command:\n      - 'printf %s \\\\{{ outs }}'\n"
)]
fn paired_backslashes_keep_the_path_as_one_argument(
    #[case] recipe: &str,
    #[case] extra: &str,
) -> Result<()> {
    if ninja_integration_workspace().is_err() {
        // Ninja is optional on a developer machine; CI sets NETSUKE_REQUIRE_NINJA=1.
        return Ok(());
    }
    let graph = lower_posix(&target_manifest(recipe, extra))?;

    let ninja = generate_posix(&graph)?;
    let commands = ninja_commands(&ninja, INJECTING_PATH)?;
    ensure!(
        commands.contains(INJECTING_PATH),
        "the generated command must carry the path text:\n{commands}"
    );

    let workspace = tempfile::tempdir()?;
    let output = Command::new("/bin/sh")
        .args(["-ec", &commands])
        .current_dir(workspace.path())
        .env_clear()
        .env("PATH", host_path()?)
        .output()
        .context("run the generated recipe command")?;
    ensure!(
        output.status.success(),
        "generated recipe command failed: {}\ncommand: {commands}",
        String::from_utf8_lossy(&output.stderr)
    );
    ensure!(
        output.stdout == format!("\\{INJECTING_PATH}").as_bytes(),
        "the path must stay one argument with its literal backslash, got: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    ensure!(
        !workspace.path().join("injected").exists(),
        "the retained backslash must not expose the injected command"
    );
    Ok(())
}
