//! Unit tests for manifest foreach expansion.

use super::*;
use minijinja::Environment;

#[path = "expand_test_cases/tracing_capture.rs"]
mod a_tracing_capture;
#[path = "expand_test_cases/condition_cases.rs"]
mod condition_cases;
#[path = "expand_test_cases/property_cases.rs"]
mod property_cases;

pub(super) fn targets(doc: &ManifestValue) -> Result<&[ManifestValue]> {
    doc.get("targets")
        .and_then(|v| v.as_array())
        .map(Vec::as_slice)
        .context("targets sequence missing")
}

pub(super) fn actions(doc: &ManifestValue) -> Result<&[ManifestValue]> {
    doc.get("actions")
        .and_then(|v| v.as_array())
        .map(Vec::as_slice)
        .context("actions sequence missing")
}

pub(super) fn section_entries<'a>(
    doc: &'a ManifestValue,
    section: &str,
) -> Result<&'a [ManifestValue]> {
    doc.get(section)
        .and_then(|v| v.as_array())
        .map(Vec::as_slice)
        .with_context(|| format!("{section} sequence missing"))
}

pub(super) fn indexes(entries: &[ManifestValue], section: &str) -> Result<Vec<u64>> {
    entries
        .iter()
        .map(|entry| -> Result<u64> {
            let map = entry
                .as_object()
                .with_context(|| format!("{section} entry map"))?;
            let vars = map
                .get("vars")
                .and_then(|v| v.as_object())
                .with_context(|| format!("{section} vars map"))?;
            let index_value = vars
                .get("index")
                .with_context(|| format!("{section} index value"))?;
            let ManifestValue::Number(num) = index_value else {
                anyhow::bail!("{section} index missing");
            };
            num.as_u64()
                .with_context(|| format!("{section} numeric index conversion failed"))
        })
        .collect()
}

#[test]
fn expand_foreach_returns_filtering_stats() -> Result<()> {
    let env = Environment::new();
    let yaml = "targets:
  - name: skipped-target
    command: echo skipped
    when: 'false'
  - name: kept-target
    command: echo kept
actions:
  - name: skipped-action
    command: echo skipped
    when: 'false'
  - name: each-action
    command: echo {{ item }}
    foreach:
      - skip
      - keep
    when: item != 'skip'";
    let mut doc: ManifestValue = serde_saphyr::from_str(yaml)?;

    let stats = expand_foreach(&mut doc, &env)?;

    anyhow::ensure!(
        stats
            == FilteringStats {
                filtered_targets: 1,
                filtered_actions: 2,
            },
        "unexpected filtering stats: {stats:?}"
    );
    anyhow::ensure!(targets(&doc)?.len() == 1, "expected one kept target");
    anyhow::ensure!(actions(&doc)?.len() == 1, "expected one kept action");
    Ok(())
}
