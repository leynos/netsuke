//! Tests for the optional target `description` field: present, absent, and
//! rejection of duplicate or unknown metadata fields alongside it.

use anyhow::{Context, Result, ensure};

use super::support::parse_manifest;

#[test]
fn target_description_is_optional() -> Result<()> {
    {
        let yaml = r#"
            netsuke_version: "1.0.0"
            targets:
              - name: hello
                description: "Build the hello binary"
                command: "echo hi"
        "#;
        let manifest = parse_manifest(yaml)?;
        let target = manifest.targets.first().context("expected target entry")?;
        ensure!(
            target.description.as_deref() == Some("Build the hello binary"),
            "unexpected target description: {:?}",
            target.description
        );
    }

    {
        let yaml = r#"
            netsuke_version: "1.0.0"
            targets:
              - name: hello
                command: "echo hi"
        "#;
        let manifest = parse_manifest(yaml)?;
        let target = manifest.targets.first().context("expected target entry")?;
        ensure!(target.description.is_none(), "description should be absent");
    }
    Ok(())
}

#[test]
fn description_duplicates_and_unknown_fields_are_rejected() -> Result<()> {
    {
        let yaml = r#"
            netsuke_version: "1.0.0"
            targets:
              - name: hello
                description: "first"
                description: "second"
                command: "echo hi"
        "#;
        let error = parse_manifest(yaml).expect_err("duplicate target description should fail");
        ensure!(
            format!("{error:?}").contains("description"),
            "duplicate-description diagnostic should name the field: {error:?}"
        );
    }

    {
        let yaml = r#"
            netsuke_version: "1.0.0"
            targets:
              - name: hello
                description: "Build it"
                explanation: "unknown metadata"
                command: "echo hi"
        "#;
        let error = parse_manifest(yaml)
            .expect_err("unknown target field alongside description should fail");
        ensure!(
            format!("{error:?}").contains("explanation"),
            "unknown-field diagnostic should name the field: {error:?}"
        );
    }
    Ok(())
}
