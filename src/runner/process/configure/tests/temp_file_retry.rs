//! Engine for the bounded Windows `%TEMP%` permission-denied retry.
//!
//! The retry policy itself — the attempt bound and the delay schedule — lives
//! beside the fixture in the parent module. This module owns the loop and the
//! deterministic tests that pin its behaviour without waiting on the real
//! clock. See "Windows temp-file fixture retries" in
//! `docs/developers-guide.md` for the evidence and the rejected alternatives.

use super::*;
use std::collections::VecDeque;

/// Run `create` until it succeeds, a non-permission error occurs, or the retry
/// budget is spent.
///
/// This is the seam the unit tests drive: `create` supplies the scripted
/// results and `before_retry` observes the attempt, the error, and the delay.
/// The hook owns the wait, so the production hook sleeps while a recording
/// test hook returns immediately.
///
/// # Errors
///
/// Returns the first error that is not a permission denial, or the last
/// permission denial once [`TEMP_FILE_ATTEMPTS`] attempts have been made. The
/// final error is returned unchanged.
pub(super) fn retry_permission_denied<T>(
    mut create: impl FnMut() -> io::Result<T>,
    mut before_retry: impl FnMut(&io::Error, u32, Duration),
) -> io::Result<T> {
    let mut attempt = 1_u32;
    loop {
        let error = match create() {
            Ok(value) => return Ok(value),
            Err(error) => error,
        };
        if error.kind() != io::ErrorKind::PermissionDenied {
            return Err(error);
        }
        let Some(delay) = retry_delay(attempt) else {
            return Err(error);
        };
        before_retry(&error, attempt, delay);
        attempt = attempt.saturating_add(1);
    }
}

/// Return the backoff that precedes the next attempt, or `None` once the
/// attempt budget is spent.
fn retry_delay(attempt: u32) -> Option<Duration> {
    let index = usize::try_from(attempt.checked_sub(1)?).ok()?;
    let millis = *TEMP_FILE_RETRY_DELAYS_MILLIS.get(index)?;
    Some(Duration::from_millis(millis))
}

#[test]
fn retry_returns_the_first_success_without_retrying() -> Result<()> {
    let mut calls = 0_u32;
    let mut retries = Vec::new();
    let value = retry_permission_denied(
        || {
            calls += 1;
            Ok(7_u32)
        },
        |error, attempt, delay| retries.push((error.kind(), attempt, delay)),
    )?;
    ensure!(
        value == 7,
        "the created value must be returned unchanged: {value}"
    );
    ensure!(
        calls == 1,
        "a first-attempt success must create once; created {calls} times"
    );
    ensure!(
        retries.is_empty(),
        "a first-attempt success must not retry: {retries:?}"
    );
    Ok(())
}

#[test]
fn retry_recovers_from_transient_permission_denials() -> Result<()> {
    let mut denials = VecDeque::from([
        io::Error::new(io::ErrorKind::PermissionDenied, "first denial"),
        io::Error::new(io::ErrorKind::PermissionDenied, "second denial"),
    ]);
    let mut calls = 0_u32;
    let mut retries = Vec::new();
    let value = retry_permission_denied(
        || {
            calls += 1;
            denials.pop_front().map_or(Ok(7_u32), Err)
        },
        |error, attempt, delay| retries.push((error.to_string(), attempt, delay)),
    )?;
    ensure!(
        value == 7,
        "the value created after the denials must be returned: {value}"
    );
    ensure!(
        calls == 3,
        "two denials then a success must create three times; created {calls} times"
    );
    let expected = vec![
        (String::from("first denial"), 1, Duration::from_millis(10)),
        (String::from("second denial"), 2, Duration::from_millis(20)),
    ];
    ensure!(
        retries == expected,
        "each denial must be announced with the next fixed delay: {retries:?}"
    );
    Ok(())
}

#[test]
fn retry_reports_the_last_permission_denial_after_the_budget() -> Result<()> {
    let mut calls = 0_u32;
    let mut retries = Vec::new();
    let error = retry_permission_denied(
        || {
            calls += 1;
            Err::<u32, _>(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("denial {calls}"),
            ))
        },
        |error, attempt, delay| retries.push((error.to_string(), attempt, delay)),
    )
    .err()
    .ok_or_else(|| anyhow::anyhow!("a persistent permission denial must be returned"))?;
    ensure!(
        calls == TEMP_FILE_ATTEMPTS,
        "the create operation must run {TEMP_FILE_ATTEMPTS} times; ran {calls} times"
    );
    ensure!(
        retries.len() == 4,
        "only the four retryable attempts may be announced; announced {retries:?}"
    );
    let observed_attempts: Vec<u32> = retries.iter().map(|(_, attempt, _)| *attempt).collect();
    ensure!(
        observed_attempts == [1, 2, 3, 4],
        "the hook must see each failed attempt number: {observed_attempts:?}"
    );
    let observed_delays: Vec<Duration> = retries.iter().map(|(_, _, delay)| *delay).collect();
    let expected_delays = [
        Duration::from_millis(10),
        Duration::from_millis(20),
        Duration::from_millis(40),
        Duration::from_millis(80),
    ];
    ensure!(
        observed_delays == expected_delays,
        "the retry delays must be fixed and jitter-free: {observed_delays:?}"
    );
    let observed_messages: Vec<&str> = retries
        .iter()
        .map(|(message, _, _)| message.as_str())
        .collect();
    ensure!(
        observed_messages == ["denial 1", "denial 2", "denial 3", "denial 4"],
        "the hook must see each failed attempt's error: {observed_messages:?}"
    );
    ensure!(
        error.kind() == io::ErrorKind::PermissionDenied,
        "the returned error must remain a permission denial: {error:?}"
    );
    ensure!(
        error.to_string() == "denial 5",
        "the fifth and final error must be returned unchanged: {error}"
    );
    Ok(())
}

#[test]
fn retry_returns_other_errors_on_the_first_attempt() -> Result<()> {
    let mut calls = 0_u32;
    let mut retries = Vec::new();
    let error = retry_permission_denied(
        || {
            calls += 1;
            Err::<u32, _>(io::Error::new(io::ErrorKind::NotFound, "no temporary name"))
        },
        |error, attempt, delay| retries.push((error.kind(), attempt, delay)),
    )
    .err()
    .ok_or_else(|| anyhow::anyhow!("a non-permission error must be returned"))?;
    ensure!(
        calls == 1,
        "a non-permission error must not be retried; created {calls} times"
    );
    ensure!(
        retries.is_empty(),
        "a non-permission error must not be announced: {retries:?}"
    );
    ensure!(
        error.kind() == io::ErrorKind::NotFound,
        "the error kind must be preserved: {error:?}"
    );
    ensure!(
        error.to_string() == "no temporary name",
        "the error must be returned unchanged: {error}"
    );
    Ok(())
}
