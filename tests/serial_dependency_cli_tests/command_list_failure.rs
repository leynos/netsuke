//! Verify target attribution for failing command lists in serial CLI builds.

use super::{cli_workspace, run_netsuke};
use crate::command_list_failure_support::{
    contains_full_fingerprint, contains_target_clause, failure_marker_suffix, identifies_entry,
};
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
        let suffix = failure_marker_suffix(&stderr)
            .context("failure output should retain the command-list marker")?;
        let (fingerprint, _) = suffix
            .split_once(", entry ")
            .context("marker should retain its fingerprint and entry index")?;
        ensure!(
            contains_full_fingerprint(&stderr),
            "diagnostic should retain the full action fingerprint: {stderr}"
        );
        ensure!(
            identifies_entry(&stderr, 2),
            "diagnostic should identify entry 2: {stderr}"
        );
        ensure!(
            contains_target_clause(&stderr, "target \"check-fmt\""),
            "diagnostic should name check-fmt: {stderr}"
        );
        ensure!(
            !contains_target_clause(&stderr, "target \"all\""),
            "the aggregate target must not replace the failing action: {stderr}"
        );
        diagnostics.push((fingerprint.to_owned(), "2".to_owned()));
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
