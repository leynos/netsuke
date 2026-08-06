//! Behavioural coverage for `NETSUKE_WHICH_WORKSPACE` at the process boundary.
//!
//! The classification rule is covered exhaustively at its pure seam
//! (`src/stdlib/which/workspace_switch.rs`); these cases prove the switch
//! observed by a real `netsuke` process changes `command_available`'s
//! answer. The variable reaches the child through `Command::env`, never by
//! mutating this process (#493), which is why the boundary test is a
//! subprocess test.

use anyhow::{Context, Result, ensure};
use test_support::fs as test_fs;
use test_support::netsuke::run_netsuke_in_with_env;

const SWITCH: &str = "NETSUKE_WHICH_WORKSPACE";

/// A manifest whose generated output encodes `command_available`'s verdict.
const MANIFEST: &str = concat!(
    "netsuke_version: \"1.0.0\"\n",
    "targets:\n",
    "  - name: \"probe-{{ command_available('netsuke_workspace_probe') }}.txt\"\n",
    "    command: \"true\"\n",
);

/// Build a workspace whose only copy of the probe lives inside the tree.
///
/// The probe is deliberately absent from the child's `PATH`, so a `true`
/// verdict can come only from the workspace fallback.
fn probe_workspace() -> Result<tempfile::TempDir> {
    let temp = tempfile::tempdir().context("create workspace")?;
    test_fs::write(temp.path().join("Netsukefile"), MANIFEST).context("write manifest")?;
    let tools = temp.path().join("tools");
    test_fs::create_dir_all(&tools).context("create tools dir")?;
    let probe = tools.join("netsuke_workspace_probe");
    test_fs::write(&probe, "#!/bin/sh\nexit 0\n").context("write probe")?;
    test_fs::set_mode(&probe, 0o755).context("chmod probe")?;
    Ok(temp)
}

/// Generate the Ninja file with `switch` applied and report the verdict.
fn probe_verdict(temp: &tempfile::TempDir, switch: Option<&str>) -> Result<String> {
    // The workspace fallback engages only when the child's PATH is empty or
    // absent, so the probe cannot be masked by host tooling; an empty PATH
    // override is therefore part of the boundary being tested.
    let mut env: Vec<(&str, &str)> = vec![("PATH", "")];
    if let Some(value) = switch {
        env.push((SWITCH, value));
    }
    let run = run_netsuke_in_with_env(temp.path(), &["generate", "--output", "out.ninja"], &env)?;
    ensure!(run.success, "generate should succeed: {}", run.stderr);
    test_fs::read_to_string(temp.path().join("out.ninja")).context("read generated ninja")
}

#[cfg(unix)]
#[test]
fn the_switch_gates_the_workspace_fallback_end_to_end() -> Result<()> {
    let temp = probe_workspace()?;

    let enabled = probe_verdict(&temp, None)?;
    ensure!(
        enabled.contains("probe-true.txt"),
        "with the switch unset the workspace fallback should find the probe:\n{enabled}"
    );

    let disabled = probe_verdict(&temp, Some("0"))?;
    ensure!(
        disabled.contains("probe-false.txt"),
        "with the switch set to 0 the fallback must not run:\n{disabled}"
    );
    Ok(())
}
