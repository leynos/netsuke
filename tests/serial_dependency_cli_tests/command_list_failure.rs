//! Verify target attribution for failing command lists in serial CLI builds.

use super::{cli_workspace, run_netsuke};
use anyhow::{Context, Result, ensure};

const FAILING_CHECK_FMT_MANIFEST: &str = concat!(
    "netsuke_version: '1.0.0'\n",
    "targets:\n",
    "  - name: check-fmt\n",
    "    command:\n",
    "      - 'true'\n",
    "      - 'false'\n",
    "      - 'touch unexpected'\n",
    "  - name: lint\n",
    "    command: 'touch lint'\n",
    "  - name: test\n",
    "    command: 'touch test'\n",
    "  - name: all\n",
    "    dependency_order: serial\n",
    "    deps: [check-fmt, lint, test]\n",
    "    command: 'touch all'\n",
    "defaults: [all]\n",
);

/// Return the diagnostic suffix after the unchanged command-list marker.
fn command_list_failure_suffix(stderr: &str) -> Option<&str> {
    stderr
        .split_once("netsuke command-list failure: action ")
        .map(|(_, suffix)| suffix.lines().next().unwrap_or_default())
}

/// Match a quoted target clause in either human or tracing output.
fn contains_target_clause(stderr: &str, clause: &str) -> bool {
    stderr.contains(clause) || stderr.contains(&clause.replace('"', "\\\""))
}

#[test]
fn serial_all_reports_the_failing_direct_target_with_or_without_progress() -> Result<()> {
    let (directory, root) = cli_workspace(FAILING_CHECK_FMT_MANIFEST)?;
    let mut diagnostics = Vec::new();

    for progress in ["always", "never"] {
        let output = run_netsuke(&directory, &["--progress", progress, "build"])?;
        ensure!(
            !output.status.success(),
            "the failing check-fmt entry must fail with progress={progress}"
        );
        let stderr = String::from_utf8(output.stderr).context("stderr should be valid UTF-8")?;
        let suffix = command_list_failure_suffix(&stderr)
            .context("failure output should retain the command-list marker")?;
        let (fingerprint, entry_and_target) = suffix
            .split_once(", entry ")
            .context("marker should retain its fingerprint and entry index")?;
        ensure!(
            fingerprint.len() == 64 && fingerprint.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "diagnostic should retain the full action fingerprint: {stderr}"
        );
        let entry = entry_and_target
            .split_whitespace()
            .next()
            .context("marker should retain its entry index")?;
        ensure!(entry == "2", "diagnostic should identify entry 2: {stderr}");
        ensure!(
            contains_target_clause(&stderr, "target \"check-fmt\""),
            "diagnostic should name check-fmt: {stderr}"
        );
        ensure!(
            !contains_target_clause(&stderr, "target \"all\""),
            "the aggregate target must not replace the failing action: {stderr}"
        );
        diagnostics.push((fingerprint.to_owned(), entry.to_owned()));
    }

    let [progress_enabled, progress_disabled] = diagnostics.as_slice() else {
        anyhow::bail!("both progress branches should produce a failure diagnostic");
    };
    ensure!(
        progress_enabled == progress_disabled,
        "progress modes should report the same fingerprint and entry: {diagnostics:?}"
    );
    for later_target in ["lint", "test", "all", "unexpected"] {
        ensure!(
            root.open(later_target).is_err(),
            "serial work continued after the check-fmt failure: {later_target}"
        );
    }
    Ok(())
}
