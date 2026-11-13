//! Tests for the `actions:` section, which defaults to phony, never-always
//! entries unless the manifest says otherwise.

use anyhow::{Context, Result, ensure};
use rstest::rstest;

use super::support::parse_manifest;

#[rstest]
#[case::default_flags(
    r#"
    netsuke_version: "1.0.0"
    actions:
      - name: setup
        command: "echo hi"
    targets:
      - name: done
        command: "true"
"#,
    true,
    false,
    false
)]
#[case::explicit_phony_false(
    r#"
    netsuke_version: "1.0.0"
    actions:
      - name: setup
        command: "echo hi"
        phony: false
    targets:
      - name: done
        command: "true"
"#,
    true,
    false,
    false
)]
#[case::explicit_console_false(
    r#"
    netsuke_version: "1.0.0"
    actions:
      - name: setup
        command: "echo hi"
        console: false
    targets:
      - name: done
        command: "true"
"#,
    true,
    false,
    false
)]
#[case::explicit_always_true(
    r#"
    netsuke_version: "1.0.0"
    actions:
      - name: setup
        command: "echo hi"
        always: true
        console: true
    targets:
      - name: done
        command: "true"
"#,
    true,
    true,
    true
)]
fn actions_behaviour(
    #[case] yaml: &str,
    #[case] expected_phony: bool,
    #[case] expected_always: bool,
    #[case] expected_console: bool,
) -> Result<()> {
    let manifest = parse_manifest(yaml)?;
    let action = manifest.actions.first().context("expected action entry")?;
    ensure!(
        action.phony == expected_phony,
        "unexpected phony flag: got {}, expected {}",
        action.phony,
        expected_phony
    );
    ensure!(
        action.always == expected_always,
        "unexpected always flag: got {}, expected {}",
        action.always,
        expected_always
    );
    ensure!(
        action.console == expected_console,
        "unexpected console flag: got {}, expected {}",
        action.console,
        expected_console
    );
    Ok(())
}

#[test]
fn action_carries_description_and_stays_phony() -> Result<()> {
    let yaml = r#"
        netsuke_version: "1.0.0"
        actions:
          - name: lint
            description: "Run rustdoc, Clippy, and Whitaker"
            command: "cargo clippy"
        targets:
          - name: done
            command: "true"
    "#;
    let manifest = parse_manifest(yaml)?;
    let action = manifest.actions.first().context("expected action entry")?;
    ensure!(
        action.description.as_deref() == Some("Run rustdoc, Clippy, and Whitaker"),
        "unexpected action description: {:?}",
        action.description
    );
    ensure!(action.phony, "actions should stay phony with a description");
    Ok(())
}

#[test]
fn multiple_actions_are_marked_phony() -> Result<()> {
    let yaml = r#"
        netsuke_version: "1.0.0"
        actions:
          - name: setup
            command: "echo hi"
          - name: build
            command: "make build"
          - name: test
            command: "cargo test"
        targets:
          - name: done
            command: "true"
    "#;
    let manifest = parse_manifest(yaml)?;
    ensure!(
        manifest.actions.len() == 3,
        "expected three actions, got {}",
        manifest.actions.len()
    );
    for action in &manifest.actions {
        ensure!(action.phony, "all actions should be phony");
        ensure!(!action.always, "actions should not always run");
    }
    Ok(())
}
