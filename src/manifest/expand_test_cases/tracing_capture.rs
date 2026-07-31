//! Tracing capture tests for manifest expansion filtering.

use super::*;
use anyhow::{Context, Result};
use minijinja::Environment;
use test_support::tracing_capture::with_test_subscriber;
use tracing_subscriber::filter::LevelFilter;

#[test]
fn expand_foreach_emits_debug_event_for_filtered_entry() -> Result<()> {
    with_test_subscriber(LevelFilter::DEBUG, |captured| {
        let env = Environment::new();
        let when_expr = "secret_token == 'literal-secret'";
        let yaml = format!(
            "targets:
  - name: skipped-target-secret
    command: echo skipped
    when: {when_expr}"
        );
        let mut doc: ManifestValue = serde_saphyr::from_str(&yaml)?;

        let stats = expand_foreach(&mut doc, &env)?;
        let events = captured.snapshot();

        anyhow::ensure!(
            stats.filtered_targets == 1,
            "expected one filtered target: {stats:?}"
        );
        let expected_hash = entry_name_hash("skipped-target-secret");
        let event = events
            .iter()
            .find(|event| {
                event.contains("filtered manifest entry by when expression")
                    && event.contains("section=\"targets\"")
                    && event.contains(&format!("entry_name_hash=\"{expected_hash}\""))
            })
            .with_context(|| format!("expected filtered-entry debug event in {events:?}"))?;
        anyhow::ensure!(
            event.contains("section=\"targets\""),
            "debug event should include section field: {event}"
        );
        anyhow::ensure!(
            event.contains(&format!("entry_name_hash=\"{expected_hash}\"")),
            "debug event should include bounded entry name hash: {event}"
        );
        anyhow::ensure!(
            event.contains(&format!("when_expression_len={}", when_expr.len())),
            "debug event should include expression length: {event}"
        );
        anyhow::ensure!(
            event.contains("when_result=false"),
            "debug event should include false when result: {event}"
        );
        anyhow::ensure!(
            !event.contains("skipped-target-secret"),
            "debug event should not include raw entry name: {event}"
        );
        anyhow::ensure!(
            !event.contains("secret_token") && !event.contains("literal-secret"),
            "debug event should not include raw when expression: {event}"
        );
        anyhow::ensure!(
            events
                .iter()
                .all(|captured_event| !captured_event.contains("entry_name=")
                    && !captured_event.contains("when_expression=")),
            "structured logs should not include raw entry_name or when_expression fields: {events:?}"
        );
        Ok(())
    })
}
