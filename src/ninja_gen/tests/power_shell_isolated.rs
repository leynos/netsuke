//! Verify fresh-process PowerShell rendering for isolated command lists.

use super::*;

use anyhow::{Context, Result, ensure};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use test_support::ninja_semantics::ResponseFileContent;

/// Decode a PowerShell `-EncodedCommand` payload as UTF-16LE source.
fn decode_power_shell_payload(payload: &str) -> Result<String> {
    let bytes = STANDARD
        .decode(payload)
        .context("decode PowerShell Base64 payload")?;
    let (pairs, remainder) = bytes.as_chunks::<2>();
    ensure!(
        remainder.is_empty(),
        "PowerShell UTF-16LE payload should contain complete code units"
    );
    let units = pairs
        .iter()
        .map(|[low, high]| u16::from(*low) | (u16::from(*high) << 8))
        .collect::<Vec<_>>();
    String::from_utf16(&units).context("decode PowerShell UTF-16LE source")
}

/// Find an encoded payload in a generated Ninja rule's command binding.
fn direct_command_payload(rendered: &str) -> Result<&str> {
    rendered
        .lines()
        .find_map(|line| line.strip_prefix("  command = "))
        .and_then(|command| {
            command
                .split_once("-EncodedCommand ")
                .map(|(_, payload)| payload)
        })
        .context("generated PowerShell command should carry an encoded payload")
}

/// Decode child payloads and verify isolated PowerShell entries run independently.
#[test]
fn isolated_power_shell_lists_use_fresh_encoded_children() -> Result<()> {
    let action = Action {
        recipe: Recipe::Command {
            command: StringOrList::List(vec![
                "$env:NETSUKE_ISOLATED = 'first'; Set-Location nested".into(),
                "Write-Output second".into(),
            ]),
            isolation: crate::ast::CommandIsolation::Isolated,
        },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    };
    let mut rendered = String::new();
    NamedAction {
        id: "isolated_power_shell_list",
        action: &action,
        shell: RecipeShell::PowerShell,
    }
    .write_into(&mut rendered)?;

    let parent = decode_power_shell_payload(direct_command_payload(&rendered)?)?;
    ensure!(parent.starts_with("$ErrorActionPreference = 'Stop'\n$LASTEXITCODE = 0\n"));
    ensure!(parent.contains("$netsukeChildExecutable = \"$PSHOME\\powershell.exe\""));
    ensure!(
        parent
            .contains("$netsukeChildCommandLineLength = 2 + $netsukeChildExecutable.Length + 1 + ")
    );
    ensure!(parent.contains("if ($netsukeChildCommandLineLength -gt 32766)"));
    ensure!(parent.contains(
        "& $netsukeChildExecutable -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -EncodedCommand "
    ));
    ensure!(parent.matches("-EncodedCommand ").count() == 2);
    ensure!(
        parent
            .matches("if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }")
            .count()
            == 3
    );
    ensure!(!parent.contains("NETSUKE_ISOLATED"));

    let children = parent
        .lines()
        .filter_map(|line| {
            line.split_once("-EncodedCommand ")
                .map(|(_, payload)| payload)
        })
        .map(decode_power_shell_payload)
        .collect::<Result<Vec<_>>>()?;
    ensure!(children.len() == 2);
    let first = children
        .first()
        .context("first isolated child should exist")?;
    let second = children
        .get(1)
        .context("second isolated child should exist")?;
    ensure!(first.starts_with("$ErrorActionPreference = 'Stop'\n$LASTEXITCODE = 0\n"));
    ensure!(first.contains("Set-Location nested"));
    ensure!(second.contains("Write-Output second"));
    ensure!(second.ends_with("if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }"));
    ensure!(
        !parent.contains("failure: action"),
        "PowerShell adds no POSIX failure markers"
    );
    Ok(())
}

/// Report an oversized isolated child with its one-based entry index and length.
#[test]
fn isolated_power_shell_rejects_child_command_lines_over_the_windows_limit() -> Result<()> {
    let action = Action {
        recipe: Recipe::Command {
            command: StringOrList::List(vec!["x".repeat(13_000), "echo later".into()]),
            isolation: crate::ast::CommandIsolation::Isolated,
        },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    };
    let error = NamedAction {
        id: "oversized_child",
        action: &action,
        shell: RecipeShell::PowerShell,
    }
    .write_into(&mut String::new())
    .expect_err("oversized child command line should fail before emission");
    ensure!(
        matches!(
            error,
            NinjaGenError::PowerShellCommandLineTooLong {
                entry_index: 1,
                actual_characters,
                maximum_characters: 32_766,
            } if actual_characters > 32_766
        ),
        "expected indexed child command-line error, got {error:?}"
    );
    Ok(())
}

/// Keep each child below its limit while the complete encoded parent uses a response file.
#[test]
fn isolated_power_shell_parent_uses_response_file_for_large_lists() -> Result<()> {
    let action = Action {
        recipe: Recipe::Command {
            command: StringOrList::List(vec!["x".repeat(5_000), "y".repeat(5_000)]),
            isolation: crate::ast::CommandIsolation::Isolated,
        },
        description: None,
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    };
    let mut rendered = String::new();
    NamedAction {
        id: "large_isolated_list",
        action: &action,
        shell: RecipeShell::PowerShell,
    }
    .write_into(&mut rendered)?;
    ensure!(rendered.contains("-File \"$rspfile\""));
    ensure!(!rendered.contains("-EncodedCommand"));
    let response = rendered
        .lines()
        .find_map(|line| line.strip_prefix("  rspfile_content = "))
        .context("large isolated list should use Ninja response-file content")?;
    let parent = ResponseFileContent::new(response).decode_power_shell_payload()?;
    ensure!(parent.matches("-EncodedCommand ").count() == 2);
    let children = parent
        .lines()
        .filter_map(|line| {
            line.split_once("-EncodedCommand ")
                .map(|(_, payload)| payload)
        })
        .map(decode_power_shell_payload)
        .collect::<Result<Vec<_>>>()?;
    ensure!(children.len() == 2);
    let first = children
        .first()
        .context("first isolated child should exist")?;
    let second = children
        .get(1)
        .context("second isolated child should exist")?;
    ensure!(first.contains(&"x".repeat(5_000)));
    ensure!(second.contains(&"y".repeat(5_000)));
    Ok(())
}
