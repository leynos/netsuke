//! Teardown coverage for [`super::guard::ScopeGuard`].
//!
//! These scenarios are about what happens *after* a body finishes or fails,
//! not about the wrapper's own behaviour: an injected error, a panic, a scope
//! that is still running, a name that must be reusable, and several scenarios
//! at once. They live apart from the wrapper assertions so each file states
//! one subject.
//!
//! Every assertion here is on an observable manager state or on the value that
//! crossed the guard. A test that only checked the guard was dropped would not
//! distinguish a teardown that ran from one that was skipped.

use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    thread,
};

use anyhow::{Context, Result, ensure};

use super::{
    guard::{with_scope, with_unit},
    harness::{
        PAYLOAD_LIFETIME, ROUTINE_RUNTIME_CAP, Retention, capture_path, run_scoped, spawn_scoped,
        unit_name,
    },
    host::prerequisites_met,
    manager::{
        capture_exists, ensure_no_failed_units, remove_capture, wait_until_active,
        wait_until_unloaded,
    },
};

/// An injected body error is returned intact and still tears the unit down.
///
/// This is the failure mode the old trailing `reset_failed` calls could not
/// cover: an early `?` return skipped them. The original error must reach the
/// caller — a cleanup failure is not allowed to replace it — and the capture
/// must survive so the failure can be explained.
#[test]
fn guard_tears_down_the_scope_after_a_body_error() -> Result<()> {
    let scenario = "error-teardown";
    if !prerequisites_met(scenario)? {
        return Ok(());
    }
    let unit = unit_name(scenario);
    let error = with_unit(unit.clone(), |scope| {
        let output = run_scoped(
            scope.unit(),
            ROUTINE_RUNTIME_CAP,
            Retention::Collect,
            "echo before-error; exit 0",
        )?;
        ensure!(
            output.status.success(),
            "the payload should exit 0 before the injected error, got {:?}",
            output.status
        );
        anyhow::bail!("injected body error for teardown coverage")
    })
    .expect_err("the injected body error must reach the caller");

    let message = format!("{error:#}");
    ensure!(
        message.contains("injected body error"),
        "the original failure must survive teardown, got: {message}"
    );
    ensure!(
        message.contains(&capture_path(&unit)),
        "the error must name the retained capture, got: {message}"
    );
    ensure!(
        capture_exists(&unit)?,
        "the capture must be retained when the body fails"
    );
    remove_capture(&unit)?;
    ensure_no_failed_units("a body error must not leave a failed scope")
}

/// A panic unwinds through the guard, which tears the unit down from `Drop`.
///
/// `Drop` is the only teardown path a panic cannot bypass. The panic payload
/// must arrive unchanged: replacing it with a cleanup error would hide the
/// defect that caused the unwind.
#[test]
#[expect(
    clippy::panic_in_result_fn,
    reason = "the panic is the scenario: it is injected inside catch_unwind to prove Drop is the teardown path a panic cannot bypass, and it never escapes this function"
)]
fn guard_tears_down_the_scope_while_a_panic_unwinds() -> Result<()> {
    let scenario = "panic-teardown";
    if !prerequisites_met(scenario)? {
        return Ok(());
    }
    let unit = unit_name(scenario);
    let unwound = catch_unwind(AssertUnwindSafe(|| {
        with_unit(unit.clone(), |scope| {
            let _ = run_scoped(
                scope.unit(),
                ROUTINE_RUNTIME_CAP,
                Retention::Collect,
                "echo before-panic; exit 0",
            )?;
            panic!("injected panic for teardown coverage");
        })
    }));
    let payload = unwound.expect_err("the injected panic must unwind through the guard");
    let message = payload
        .downcast_ref::<&str>()
        .copied()
        .unwrap_or("<a non-string panic payload>");
    ensure!(
        message.contains("injected panic"),
        "the panic payload must arrive unchanged, got {message:?}"
    );
    ensure_no_failed_units("a panic must not leave a failed scope")
}

/// The guard stops a scope that is still running and reaps its launcher.
///
/// The body returns while the payload is still active, so teardown has to
/// stop a live unit rather than reset a finished one. The launcher is waited
/// on afterwards: a stopped scope must not report success, and leaving the
/// child unreaped would hide whether it ever exited.
#[test]
fn guard_stops_an_active_scope_and_reaps_the_launcher() -> Result<()> {
    let scenario = "active-teardown";
    if !prerequisites_met(scenario)? {
        return Ok(());
    }
    let unit = unit_name(scenario);
    let mut spawned = None;
    with_unit(unit.clone(), |scope| {
        let child = spawn_scoped(
            scope.unit(),
            ROUTINE_RUNTIME_CAP,
            Retention::Collect,
            &format!("sleep {}", PAYLOAD_LIFETIME.as_secs()),
        )?;
        wait_until_active(scope.unit())?;
        spawned = Some(child);
        Ok(())
    })?;

    let mut launcher = spawned.context("the body must have spawned the launcher")?;
    let status = launcher.wait().context("reap the stopped launcher")?;
    ensure!(
        !status.success(),
        "a scope stopped by teardown should not report success to its launcher, got {status:?}"
    );
    ensure_no_failed_units("stopping an active scope must leave no failed unit")
}

/// A unit name can be reused once its scope has been torn down.
///
/// This is the observable proof that teardown really unloaded the unit: a
/// second `systemd-run --unit` under the same name fails outright if the first
/// unit is still loaded.
#[test]
fn a_unit_name_can_be_reused_after_teardown() -> Result<()> {
    let scenario = "reused-name";
    if !prerequisites_met(scenario)? {
        return Ok(());
    }
    let unit = unit_name(scenario);
    for attempt in 1..=2 {
        with_unit(unit.clone(), |scope| {
            let output = run_scoped(
                scope.unit(),
                ROUTINE_RUNTIME_CAP,
                Retention::Collect,
                "echo reused; exit 0",
            )?;
            ensure!(
                output.status.success(),
                "attempt {attempt} under one name should succeed, got {:?}: a name can \
                 only be reused when the previous unit really unloaded",
                output.status
            );
            Ok(())
        })?;
        // Collection is asynchronous, so the second launch waits for the first
        // unit to leave the manager. A teardown that left it loaded for good
        // would fail here rather than being mistaken for a passing run.
        wait_until_unloaded(&unit)?;
    }
    ensure_no_failed_units("reusing a name must leave no failed unit")
}

/// Concurrent scenarios must not collide or accumulate failures.
///
/// Each thread derives its own name from the same counter, so a collision
/// would surface as a `systemd-run` failure rather than as silent interference.
/// The read-only glob afterwards covers every unit this process created, so a
/// leak from any one thread fails the scenario.
#[test]
fn concurrent_scopes_leave_no_failed_units() -> Result<()> {
    let scenario = "concurrent";
    if !prerequisites_met(scenario)? {
        return Ok(());
    }
    let failures = run_concurrently(scenario, 3);
    ensure!(
        failures.is_empty(),
        "every concurrent scenario should succeed, but {} failed: {failures:#?}",
        failures.len()
    );
    ensure_no_failed_units("concurrent runs must leave no failed unit")
}

/// Run `count` scenarios at once and return each failure, as a rendered report.
///
/// The scenario body is its own function so the thread's closure is a call
/// rather than a nested block: an outcome collected inline here would put the
/// spawn, the body and the assertion four levels deep.
fn run_concurrently(scenario: &str, count: usize) -> Vec<String> {
    thread::scope(|threads| {
        let handles: Vec<_> = (0..count)
            .map(|index| threads.spawn(move || run_one_of(scenario, index)))
            .collect();
        handles
            .into_iter()
            .filter_map(|handle| match handle.join() {
                Ok(Ok(())) => None,
                Ok(Err(err)) => Some(format!("{err:#}")),
                Err(_) => Some("a concurrent scenario panicked".to_owned()),
            })
            .collect()
    })
}

/// Run one concurrent scenario under its own name.
fn run_one_of(scenario: &str, index: usize) -> Result<()> {
    let name = format!("{scenario}-{index}");
    with_scope(&name, |scope| {
        let output = run_scoped(
            scope.unit(),
            ROUTINE_RUNTIME_CAP,
            Retention::Collect,
            "echo concurrent; exit 0",
        )?;
        ensure!(
            output.status.success(),
            "concurrent scenario {index} should exit 0, got {:?}",
            output.status
        );
        Ok(())
    })
}
