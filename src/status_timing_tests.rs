//! Property-based state-machine tests for verbose timing summaries.

use super::*;
use crate::output_prefs;
use proptest::prelude::*;
use std::io::{self, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use test_support::fluent::normalize_fluent_isolates;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecordedEvent {
    Stage,
    Progress,
    Complete,
    Summary,
}

#[derive(Debug)]
struct EventRecordingReporter {
    events: Arc<Mutex<Vec<RecordedEvent>>>,
}

impl StatusReporter for EventRecordingReporter {
    fn report_stage(&self, _current: StageNumber, _total: StageNumber, _description: &str) {
        record_event(&self.events, RecordedEvent::Stage);
    }

    fn report_task_progress(&self, _current: u32, _total: u32, _description: &str) {
        record_event(&self.events, RecordedEvent::Progress);
    }

    fn report_complete(&self, _tool_key: LocalizationKey) {
        record_event(&self.events, RecordedEvent::Complete);
    }
}

#[derive(Debug)]
struct EventRecordingWriter {
    buffer: Arc<Mutex<Vec<u8>>>,
    events: Arc<Mutex<Vec<RecordedEvent>>>,
    summary_started: bool,
}

impl Write for EventRecordingWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if !self.summary_started {
            record_event(&self.events, RecordedEvent::Summary);
            self.summary_started = true;
        }
        self.buffer
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone)]
enum StatusUpdate {
    Stage {
        current: u32,
        description: String,
    },
    Progress {
        current: u32,
        total: u32,
        description: String,
    },
}

#[derive(Debug, Clone)]
enum StatusOperation {
    Update(StatusUpdate),
    Complete,
}

fn record_event(events: &Arc<Mutex<Vec<RecordedEvent>>>, event: RecordedEvent) {
    events
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .push(event);
}

fn status_update_strategy() -> impl Strategy<Value = StatusUpdate> {
    prop_oneof![
        (1_u32..=6, "[a-z]{0,16}").prop_map(|(current, description)| StatusUpdate::Stage {
            current,
            description
        }),
        (0_u32..=8, 1_u32..=8, "[a-z]{0,16}").prop_map(|(current, total, description)| {
            StatusUpdate::Progress {
                current,
                total,
                description,
            }
        }),
    ]
}

fn status_operation_strategy() -> impl Strategy<Value = StatusOperation> {
    prop_oneof![
        status_update_strategy().prop_map(StatusOperation::Update),
        Just(StatusOperation::Complete)
    ]
}

fn apply_update(reporter: &dyn StatusReporter, update: &StatusUpdate) {
    match update {
        StatusUpdate::Stage {
            current,
            description,
        } => reporter.report_stage(
            StageNumber::new_unchecked(*current),
            StageNumber::new_unchecked(6),
            description,
        ),
        StatusUpdate::Progress {
            current,
            total,
            description,
        } => reporter.report_task_progress(*current, *total, description),
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 64, .. ProptestConfig::default() })]

    #[test]
    fn verbose_timing_reporter_preserves_completion_state_machine(
        first_description in "[a-z]{0,16}",
        before_complete in prop::collection::vec(status_update_strategy(), 0..12),
        after_complete in prop::collection::vec(status_operation_strategy(), 0..12),
    ) {
        let events = Arc::new(Mutex::new(Vec::new()));
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let clock_ticks = Arc::new(AtomicUsize::new(0));
        let reporter_clock = Arc::clone(&clock_ticks);
        let reporter = VerboseTimingReporter::with_clock_and_writer(
            Box::new(EventRecordingReporter { events: Arc::clone(&events) }),
            output_prefs::resolve_with(None, |_| None),
            Box::new(move || Duration::from_millis(reporter_clock.fetch_add(1, Ordering::SeqCst) as u64)),
            EventRecordingWriter { buffer: Arc::clone(&buffer), events: Arc::clone(&events), summary_started: false },
        );

        let first_stage = StatusUpdate::Stage { current: 1, description: first_description };
        let mut expected_forwarded = vec![RecordedEvent::Stage];
        apply_update(&reporter, &first_stage);
        for update in &before_complete {
            apply_update(&reporter, update);
            expected_forwarded.push(match update {
                StatusUpdate::Stage { .. } => RecordedEvent::Stage,
                StatusUpdate::Progress { .. } => RecordedEvent::Progress,
            });
        }

        reporter.report_complete(LocalizationKey::new(keys::STATUS_TOOL_GENERATE));
        expected_forwarded.push(RecordedEvent::Complete);
        for operation in &after_complete {
            match operation {
                StatusOperation::Update(update) => apply_update(&reporter, update),
                StatusOperation::Complete => reporter.report_complete(LocalizationKey::new(keys::STATUS_TOOL_GENERATE)),
            }
        }

        let observed = events.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone();
        let forwarded = observed.iter().copied().filter(|event| *event != RecordedEvent::Summary).collect::<Vec<_>>();
        prop_assert_eq!(forwarded, expected_forwarded);
        prop_assert_eq!(observed.iter().filter(|event| **event == RecordedEvent::Complete).count(), 1);
        prop_assert_eq!(observed.iter().filter(|event| **event == RecordedEvent::Summary).count(), 1);
        let completion_index = observed.iter().position(|event| *event == RecordedEvent::Complete).expect("the forced completion should reach the inner reporter");
        let summary_index = observed.iter().position(|event| *event == RecordedEvent::Summary).expect("the injected writer should observe one timing summary");
        prop_assert!(completion_index < summary_index);

        let output = buffer.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        let rendered = normalize_fluent_isolates(&String::from_utf8_lossy(&output));
        prop_assert!(rendered.contains("Stage timing summary:"));
    }
}
