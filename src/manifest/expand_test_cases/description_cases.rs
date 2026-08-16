//! Expansion cases proving a target or action `description` key survives
//! `foreach` expansion and is dropped together with filtered entries.

use super::*;
use anyhow::{Context, Result};
use minijinja::Environment;
use rstest::rstest;

#[rstest]
#[case::targets("targets")]
#[case::actions("actions")]
fn expand_static_entry_preserves_description(#[case] section: &str) -> Result<()> {
    let env = Environment::new();
    let yaml = format!(
        "{section}:
  - name: report
    description: Build the report
    command: echo report"
    );
    let mut doc: ManifestValue = serde_saphyr::from_str(&yaml)?;
    expand_foreach(&mut doc, &env)?;
    let entries = section_entries(&doc, section)?;
    anyhow::ensure!(entries.len() == 1, "expected one {section} entry");
    let map = entries
        .first()
        .and_then(ManifestValue::as_object)
        .with_context(|| format!("{section} entry map"))?;
    let description = map
        .get("description")
        .and_then(ManifestValue::as_str)
        .with_context(|| format!("{section} description"))?;
    anyhow::ensure!(
        description == "Build the report",
        "description should be carried through expansion: {description}"
    );
    Ok(())
}

#[rstest]
#[case::targets("targets")]
#[case::actions("actions")]
fn expand_foreach_descriptions_are_rendered_with_item(#[case] section: &str) -> Result<()> {
    let env = Environment::new();
    // The `foreach` list is a local sequence, so expansion clones the whole
    // entry map including the `description` key with its `{{ item }}` template.
    let yaml = format!(
        "{section}:
  - name: report-{{{{ item }}}}
    description: Build the {{{{ item }}}} report
    command: echo {{{{ item }}}}
    foreach:
      - weekly
      - monthly
      - annual"
    );
    let mut doc: ManifestValue = serde_saphyr::from_str(&yaml)?;
    expand_foreach(&mut doc, &env)?;
    let entries = section_entries(&doc, section)?;
    anyhow::ensure!(
        entries.len() == 3,
        "expected three expanded {section} entries"
    );
    let descriptions: Result<Vec<_>> = entries
        .iter()
        .map(|entry| {
            entry
                .as_object()
                .and_then(|map| map.get("description"))
                .and_then(ManifestValue::as_str)
                .map(str::to_owned)
                .with_context(|| format!("{section} description"))
        })
        .collect();
    let expected = vec![
        "Build the {{ item }} report".to_owned(),
        "Build the {{ item }} report".to_owned(),
        "Build the {{ item }} report".to_owned(),
    ];
    anyhow::ensure!(
        descriptions? == expected,
        "description templates should survive expansion for later rendering"
    );
    Ok(())
}

#[rstest]
#[case::targets("targets")]
#[case::actions("actions")]
fn expand_when_filter_drops_description_with_the_entry(#[case] section: &str) -> Result<()> {
    let env = Environment::new();
    let yaml = format!(
        "{section}:
  - name: skipped
    description: Should vanish
    command: echo skipped
    when: 'false'
  - name: kept
    description: Should remain
    command: echo kept"
    );
    let mut doc: ManifestValue = serde_saphyr::from_str(&yaml)?;
    expand_foreach(&mut doc, &env)?;
    let entries = section_entries(&doc, section)?;
    anyhow::ensure!(
        entries.len() == 1,
        "expected exactly one kept {section} entry"
    );
    let map = entries
        .first()
        .and_then(ManifestValue::as_object)
        .with_context(|| format!("{section} entry map"))?;
    let description = map
        .get("description")
        .and_then(ManifestValue::as_str)
        .with_context(|| format!("{section} description"))?;
    anyhow::ensure!(
        description == "Should remain",
        "kept {section} description should survive: {description}"
    );
    Ok(())
}
