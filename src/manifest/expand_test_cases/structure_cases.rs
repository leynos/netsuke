//! Structural preservation cases for manifest foreach expansion.

use super::*;
use anyhow::{Context, Result};
use minijinja::Environment;

#[test]
fn expand_foreach_preserves_object_key_order() -> Result<()> {
    let env = Environment::new();
    let yaml = r"targets:
  - name: literal
    vars:
      existing: keep
    foreach:
      - 1
      - 2
    when: 'true'
    after: done
";
    let mut doc: ManifestValue = serde_saphyr::from_str(yaml)?;
    expand_foreach(&mut doc, &env)?;
    let targets = targets(&doc)?;
    anyhow::ensure!(targets.len() == 2, "expected expanded targets");
    for target in targets {
        let map = target.as_object().context("target object")?;
        let keys: Vec<&str> = map.keys().map(String::as_str).collect();
        anyhow::ensure!(
            keys == ["name", "vars", "after"],
            "key order should remain stable: {:?}",
            keys
        );
    }
    Ok(())
}
