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
    guard::with_unit,
    harness::{
        PAYLOAD_LIFETIME, ROUTINE_RUNTIME_CAP, RUNTIME_CAP, Retention, capture_path, run_scoped,
        spawn_scoped, unit_name,
    },
    host::prerequisites_met,
    manager::{
        capture_exists, ensure_own_unit_settled, remove_capture, wait_for_terminal,
        wait_until_active, wait_until_unloaded,
    },
};

/// Leave `unit` failed and loaded, so only the guard can clear it.
///
/// The exceptional-path scenarios need this rather than a payload that exits
/// successfully. A scope that finishes cleanly is collected by the manager on
/// its own, so such a scenario would pass even if the guard did nothing at all.
/// A scope stopped at the runtime cap under `Retention::Retain` is left
/// `failed` with its `Result` intact, which is exactly the state only the
/// guard's teardown clears. Waiting for the terminal state makes the premise
/// true before the error is injected rather than racing it.
fn leave_scope_failed(unit: &str) -> Result<()> {
    let _ = run_scoped(
        unit,
        RUNTIME_CAP,
        Retention::Retain,
        &format!("trap '' TERM; sleep {}", PAYLOAD_LIFETIME.as_secs()),
    )?;
    let _ = wait_for_terminal(unit)?;
    Ok(())
}

/// An injected body error is returned intact and still tears the unit down.
///
/// This is the failure mode the old trailing `reset_failed` calls could not
/// cover: an early `?` return skipped them. The original error must reach the
/// caller — a cleanup failure is not allowed to replace it — and the capture
/// must survive so the failure can be explained.
///
/// The scope is left failed and retained before the error is injected, so the
/// assertion that it settles can only pass if the guard really acted. A
/// successful scope would be collected by the manager either way, and the test
/// would then prove nothing about the guard.
#[test]
fn guard_tears_down_the_scope_after_a_body_error() -> Result<()> {
    let scenario = "error-teardown";
    if !prerequisites_met(scenario)? {
        return Ok(());
    }
    let unit = unit_name(scenario);
    let error = with_unit(unit.clone(), |scope| {
        leave_scope_failed(scope.unit())?;
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
    ensure_own_unit_settled(&unit, "a body error must not leave its scope failed")
}

/// A panic unwinds through the guard, which tears the unit down from `Drop`.
///
/// `Drop` is the only teardown path a panic cannot bypass. The panic payload
/// must arrive unchanged: replacing it with a cleanup error would hide the
/// defect that caused the unwind.
///
/// As in the injected-error case, the scope is left failed and retained before
/// the panic, so the assertion that it settles distinguishes a guard that ran
/// from one that did not.
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
            leave_scope_failed(scope.unit())?;
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
    // `Drop` stops and resets the unit but never deletes the capture: during an
    // unwind it cannot raise, and the file is the evidence a failing run needs.
    // A passing run therefore has to remove it, which the body-error test does
    // explicitly on its success path too.
    ensure_own_unit_settled(&unit, "a panic must not leave its scope failed")?;
    remove_capture(&unit)
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
    let outcome = with_unit(unit.clone(), |scope| {
        // Record the child before anything else can fail. Dropping a `Child`
        // neither stops nor waits for it, so a probe that returned early would
        // leave the launcher running: it could register its scope *after*
        // teardown had looked and found nothing, and the unit would outlive its
        // guard. The assignment is what lets the reaping below happen.
        let child = spawn_scoped(
            scope.unit(),
            ROUTINE_RUNTIME_CAP,
            Retention::Collect,
            &format!("sleep {}", PAYLOAD_LIFETIME.as_secs()),
        )?;
        spawned = Some(child);
        wait_until_active(scope.unit())?;
        Ok(())
    });

    // Reap the launcher whether or not the body succeeded, so a failed probe
    // cannot leave a process behind. The status is only meaningful when the
    // body reached its assertions.
    let reaped = match &mut spawned {
        Some(child) => Some(child.wait().context("reap the stopped launcher")?),
        None => None,
    };
    outcome?;
    let exit = reaped.context("the body must have spawned the launcher")?;
    ensure!(
        !exit.success(),
        "a scope stopped by teardown should not report success to its launcher, got {exit:?}"
    );
    ensure_own_unit_settled(&unit, "stopping an active scope must leave no failed unit")
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
    ensure_own_unit_settled(&unit, "reusing a name must leave no failed unit")
}

/// Concurrent scenarios must not collide or accumulate failures.
///
/// Each thread derives its own name from the same counter, so a collision
/// would surface as a `systemd-run` failure rather than as silent interference.
/// Each unit is then checked by name, so a leak from any one thread fails the
/// scenario and names the unit that leaked.
#[test]
fn concurrent_scopes_leave_no_failed_units() -> Result<()> {
    let scenario = "concurrent";
    if !prerequisites_met(scenario)? {
        return Ok(());
    }
    let (units, failures) = run_concurrently(scenario, 3);
    ensure!(
        failures.is_empty(),
        "every concurrent scenario should succeed, but {} failed: {failures:#?}",
        failures.len()
    );
    for unit in &units {
        ensure_own_unit_settled(
            unit,
            &format!("concurrent run {unit} must leave no failed unit"),
        )?;
    }
    Ok(())
}

/// Run `count` scenarios at once, returning each unit's name and each failure.
///
/// The names are allocated before the threads start, so the caller checks the
/// units that were really created rather than re-deriving names from a counter
/// that has since moved on. The scenario body is its own function so the
/// thread's closure is a call rather than a nested block.
fn run_concurrently(scenario: &str, count: usize) -> (Vec<String>, Vec<String>) {
    let names: Vec<String> = (0..count)
        .map(|index| unit_name(&format!("{scenario}-{index}")))
        .collect();
    let failures = thread::scope(|threads| {
        // The borrow is scoped to this `thread::scope`, so each thread can read
        // its name from `names` rather than owning a clone of it.
        let handles: Vec<_> = names
            .iter()
            .map(|unit| threads.spawn(move || run_one_of(unit)))
            .collect();
        handles
            .into_iter()
            .filter_map(|handle| match handle.join() {
                Ok(Ok(())) => None,
                Ok(Err(err)) => Some(format!("{err:#}")),
                Err(_) => Some("a concurrent scenario panicked".to_owned()),
            })
            .collect()
    });
    (names, failures)
}

/// Run one concurrent scenario under the unit name it was given.
fn run_one_of(unit: &str) -> Result<()> {
    with_unit(unit.to_owned(), |scope| {
        let output = run_scoped(
            scope.unit(),
            ROUTINE_RUNTIME_CAP,
            Retention::Collect,
            "echo concurrent; exit 0",
        )?;
        ensure!(
            output.status.success(),
            "{unit} should exit 0, got {:?}",
            output.status
        );
        Ok(())
    })
}
