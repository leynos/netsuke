//! Connection acceptance for the local HTTP fixture.
//!
//! The fixture listener is non-blocking, so accepting a client is a polling
//! loop rather than one call. `mod.rs` owns the configuration that decides how
//! long a fixture waits; this module owns the wait itself and the retry rules
//! that make it safe to poll. Nothing outside the fixture reaches it.

use std::{
    io,
    net::{TcpListener, TcpStream},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::{Duration, Instant},
};

/// How long the fixture waits for its next client connection.
#[derive(Debug, Clone, Copy)]
pub(super) enum AcceptWait<'a> {
    /// Fail the fixture when no client connects before this instant.
    Until(Instant),
    /// Wait for a client until `shutdown` reports a request to stop.
    ///
    /// Used by fixtures that expect no request, so that a slow machine fails
    /// nothing: the wait ends when the test joins the fixture, whenever that
    /// join chooses to say so.
    UntilShutdown(&'a AtomicBool),
}

impl AcceptWait<'_> {
    /// Return the instant after which this wait fails, if it is bounded.
    pub(super) const fn deadline(self) -> Option<Instant> {
        match self {
            Self::Until(deadline) => Some(deadline),
            Self::UntilShutdown(_) => None,
        }
    }

    /// Return whether the fixture has been asked to stop accepting.
    ///
    /// This is the shutdown condition, and is deliberately independent of any
    /// connection: the wake-up a join sends only shortens the wait, so a
    /// wake-up that never arrives must not be able to strand it.
    fn is_shutdown(&self) -> bool {
        match self {
            Self::Until(_) => false,
            Self::UntilShutdown(shutdown) => shutdown.load(Ordering::Acquire),
        }
    }
}

/// Return whether `deadline` has passed.
fn is_past_deadline(deadline: Instant) -> bool {
    Instant::now() >= deadline
}

/// Return whether an accept error is transient and still within the deadline.
fn should_retry_accept(
    err: &io::Error,
    wait: AcceptWait<'_>,
    poll_interval: Duration,
    accept_timeout: Duration,
) -> bool {
    if let Some(deadline) = wait.deadline() {
        assert!(
            !is_past_deadline(deadline),
            "timed out waiting for fetch test connection (accept_timeout={accept_timeout:?}, poll_interval={poll_interval:?})"
        );
    }
    // Treat transient readiness states (EAGAIN/EWOULDBLOCK) and EINTR as retryable.
    matches!(
        err.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
    )
}

/// Return the time remaining until `deadline`, never negative.
fn remaining_until_deadline(deadline: Instant) -> Duration {
    let now = Instant::now();
    if deadline > now {
        deadline - now
    } else {
        Duration::from_millis(0)
    }
}

/// Accept a client, retrying transient errors until `wait` is satisfied.
///
/// Returns `None` when the fixture is shut down before a client connects. A
/// caller that treats `None` as a finished run therefore ends the accept loop
/// on the shutdown signal alone, without depending on the wake-up connection
/// actually arriving.
#[expect(
    clippy::panic,
    reason = "tests panic when the helper cannot accept a client"
)]
pub(super) fn accept_connection(
    listener: &TcpListener,
    wait: AcceptWait<'_>,
    poll_interval: Duration,
    accept_timeout: Duration,
) -> Option<TcpStream> {
    // The shutdown check belongs in the loop condition, not in its body: a
    // nested `if` adds a second depth-2 conditional block, which CodeScene's
    // Bumpy Road biomarker flags. The two forms are otherwise equivalent.
    while !wait.is_shutdown() {
        match listener.accept() {
            Ok((stream, _)) => return Some(stream),
            Err(err) if should_retry_accept(&err, wait, poll_interval, accept_timeout) => {
                let nap = wait.deadline().map_or(poll_interval, |deadline| {
                    remaining_until_deadline(deadline).min(poll_interval)
                });
                thread::sleep(nap);
            }
            Err(err) => panic!("failed to accept connection: {err}"),
        }
    }
    None
}
