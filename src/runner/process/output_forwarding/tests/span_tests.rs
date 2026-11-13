//! Verify a forwarding warning is attributed to the Ninja command span.
//!
//! The stderr forwarding thread starts outside the caller's thread-local
//! `ninja_subprocess` span, so the span is carried into the thread and entered
//! there. These cases drive the same reporting path inside a real command span
//! and assert the warning, its fields, and its enclosing scope.

use super::super::report_forward_failure;
use super::support::RejectingWriter;
use crate::runner::process::{
    StderrMode,
    logging::{CommandLogContext, command_span},
    streaming::{ForwardRoute, ForwardStream, forward_child_output},
};
use crate::test_tracing_capture::with_test_subscriber;
use std::{io::Cursor, process::Command};
use tracing_subscriber::filter::LevelFilter;

/// Name of the command span the forwarding thread must remain inside.
const COMMAND_SPAN: &str = "ninja_subprocess";

#[test]
fn a_forwarding_warning_is_reported_inside_the_command_span() {
    let (events, scopes, span_fields) = with_test_subscriber(LevelFilter::TRACE, |captured| {
        let context = CommandLogContext::from_command(&Command::new("ninja"));
        let span = command_span(&context, "build", StderrMode::Forward);
        span.in_scope(|| {
            let mut report = report_forward_failure;
            forward_child_output(
                Cursor::new(b"lost".to_vec()),
                RejectingWriter,
                ForwardRoute::new(ForwardStream::Stderr, &mut report),
            );
        });
        (
            captured.snapshot(),
            captured.event_spans(),
            captured.span_fields(COMMAND_SPAN),
        )
    });

    let [event] = events.as_slice() else {
        panic!("expected one forwarding warning, got {events:?}");
    };
    assert!(
        event.contains("stream=\"stderr\"") && event.contains("failure_kind=\"write\""),
        "the warning must carry its bounded labels: {event}"
    );
    assert!(
        event.contains("error="),
        "the warning must carry the underlying error: {event}"
    );
    assert_eq!(
        scopes,
        vec![vec![COMMAND_SPAN.to_owned()]],
        "the warning must be recorded under the command span"
    );
    assert!(
        span_fields
            .iter()
            .any(|field| field == "operation=\"build\""),
        "the command span must carry the invocation context: {span_fields:?}"
    );
}
