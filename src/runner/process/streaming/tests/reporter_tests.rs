//! Verify the injected failure reporter receives each classification.
//!
//! These cases pass their own reporter instead of the shared counter, so the
//! reporting contract is observed directly: the copy loop must hand over what
//! it classified, and nothing else.

use super::super::{
    ForwardFailure, ForwardFailureKind, ForwardRoute, ForwardStream, forward_child_output,
};
use super::failure_tests::{ClosedWriter, FailsWhileDraining};
use std::io::Cursor;

#[test]
fn the_injected_reporter_receives_a_write_failure() {
    let mut seen = Vec::new();
    {
        let mut report = |stream: ForwardStream, failure: &ForwardFailure| {
            seen.push((stream, failure.kind));
        };
        let stats = forward_child_output(
            Cursor::new(b"lost".to_vec()),
            ClosedWriter,
            ForwardRoute::new(ForwardStream::Stdout, &mut report),
        );
        assert_eq!(stats.failure, Some(ForwardFailureKind::Write));
    }

    assert_eq!(
        seen,
        vec![(ForwardStream::Stdout, ForwardFailureKind::Write)],
        "the reporter, not the copy loop, must receive the classification"
    );
}

#[test]
fn the_injected_reporter_receives_a_drain_failure_separately() {
    let mut seen = Vec::new();
    {
        let mut report = |stream: ForwardStream, failure: &ForwardFailure| {
            seen.push((stream, failure.kind));
        };
        let stats = forward_child_output(
            FailsWhileDraining {
                bytes: b"first".to_vec(),
                delivered: false,
            },
            ClosedWriter,
            ForwardRoute::new(ForwardStream::Stderr, &mut report),
        );
        assert!(stats.drain_failed, "the drain must fail in this case");
    }

    assert_eq!(
        seen,
        vec![
            (ForwardStream::Stderr, ForwardFailureKind::Write),
            (ForwardStream::Stderr, ForwardFailureKind::Drain),
        ],
        "a drain failure must arrive as its own classification"
    );
}
