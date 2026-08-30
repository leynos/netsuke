//! Telemetry coverage for expansion reports emitted by manifest loading.

use super::super::from_str;
use crate::test_tracing_capture::with_test_subscriber;
use anyhow::{Context, Result, ensure};
use test_support::manifest::manifest_yaml;
use tracing::level_filters::LevelFilter;

/// Verify that manifest loading emits bounded filtering telemetry.
#[test]
fn manifest_loading_traces_filtered_entries_and_summary() -> Result<()> {
    let yaml = manifest_yaml(
        "targets:
  - name: skipped-target
    command: echo skipped
    when: 'false'
actions:
  - name: skipped-action
    command: echo skipped
    when: 'false'",
    );

    with_test_subscriber(LevelFilter::DEBUG, |captured| {
        let manifest = from_str(&yaml)?;
        let events = captured.snapshot();

        ensure!(
            manifest.targets.is_empty(),
            "filtered target must be removed"
        );
        ensure!(
            manifest.actions.is_empty(),
            "filtered action must be removed"
        );
        for (section, hash) in [("targets", "63563386"), ("actions", "b61bdf58")] {
            let event = events
                .iter()
                .find(|event| {
                    event.contains("filtered manifest entry by when expression")
                        && event.contains(&format!("section=\"{section}\""))
                        && event.contains(&format!("entry_name_hash=\"{hash}\""))
                })
                .with_context(|| format!("missing {section} filtering event in {events:?}"))?;
            ensure!(
                event.contains("when_expression_len=5") && event.contains("when_result=false"),
                "filtering event must preserve bounded metadata: {event}"
            );
        }
        let summary = events
            .iter()
            .find(|event| event.contains("expanded manifest foreach and when directives"))
            .context("missing expansion summary event")?;
        ensure!(
            summary.contains("filtered_targets=1")
                && summary.contains("filtered_actions=1")
                && summary.contains("filtered_entry_count=2")
                && summary.contains("omitted_filtered_entries=0"),
            "summary must report exact aggregate filtering counts: {summary}"
        );
        ensure!(
            events.iter().all(|event| !event.contains("skipped-target")
                && !event.contains("skipped-action")
                && !event.contains("when_expression=")),
            "telemetry must not disclose raw filtering inputs: {events:?}"
        );
        Ok(())
    })
}
