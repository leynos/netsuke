//! Check JSON output and configuration-error output.
//!
//! JSON mode suppresses the status reporter and every tracing event, so its
//! bytes carry no escape at all. A configuration-load failure is the awkward
//! case: it happens before the configuration that would select a colour policy
//! has been read, so the human-readable message stays plain under every policy
//! while the structured tracing events beside it follow the startup colour
//! hint. Both halves are asserted here, because a change that styled the
//! message or unstyled the events would break one contract and satisfy the
//! other.

use super::support::{TerminalWorkspace, visible_text};
use anyhow::{Context, Result, ensure};
use googletest::prelude::*;
use rstest::rstest;
use test_support::{fs, terminal::classify};

/// Write a malformed configuration file into `workspace`.
fn stage_broken_config(workspace: &TerminalWorkspace) -> Result<()> {
    fs::write(workspace.root_path().join("broken.toml"), "not = [valid\n")
        .context("write malformed configuration")?;
    Ok(())
}

/// Return the stderr lines that carry the user-facing configuration error.
fn error_text_lines(stderr: &[u8]) -> Vec<String> {
    visible_text(stderr)
        .lines()
        .filter(|line| line.contains("broken.toml") || line.contains("unclosed array"))
        .map(str::to_owned)
        .collect()
}

/// Pre-merge output splits into plain error text and policy-styled tracing.
///
/// A configuration-load failure happens before any configuration value can
/// select a colour policy, so the message Netsuke wrote for the user is plain
/// under every policy — that is the guarantee the users' guide gives. The
/// structured `tracing` events emitted alongside it are a different thing:
/// `D11` puts them under `--color`, and the docs say so. This test pins both
/// halves and their non-vacuity, because asserting only "plain" would hide a
/// regression that dropped tracing style, and asserting only "styled" would
/// break the user-facing guarantee.
#[googletest::gtest]
#[rstest]
#[case::colour_always("always", true)]
#[case::colour_never("never", false)]
fn configuration_failure_text_is_plain_while_tracing_follows_colour(
    #[case] colour: &str,
    #[case] tracing_has_sgr: bool,
) -> Result<()> {
    let workspace = TerminalWorkspace::new()?;
    stage_broken_config(&workspace)?;
    let output = workspace
        .run_piped(
            &["--config", "broken.toml", "--color", colour, "generate"],
            &[],
        )
        .context("run command with malformed configuration")?;
    ensure!(
        !output.status.success(),
        "malformed configuration should fail before execution"
    );

    let text_lines = error_text_lines(&output.stderr);
    ensure!(
        !text_lines.is_empty(),
        "the run must name the malformed file, got:\n{}",
        visible_text(&output.stderr)
    );
    for line in &text_lines {
        ensure!(
            !line.contains('\u{1b}'),
            "pre-merge error text must stay plain under --color {colour}: {line:?}"
        );
    }

    expect_that!(classify(&output.stdout).has_any_escape, eq(false));
    expect_that!(classify(&output.stderr).has_sgr, eq(tracing_has_sgr));
    Ok(())
}
