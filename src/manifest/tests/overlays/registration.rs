//! Behaviour a legitimate overlay must have once it is registered.
//!
//! An overlay is only useful if expansion sees it everywhere a helper would be
//! seen — `foreach`, `when`, and rendered fields — and if a replaced helper
//! stays reachable through [`OverlayCall::shadowed`] instead of being lost.

use std::error::Error as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context as _, Result, ensure};
use minijinja::value::Value;
use minijinja::{Error as MiniJinjaError, ErrorKind};

use super::{constant_overlay, load, load_with, rendered_command, target_names};
use crate::manifest::{OverlayCall, OverlayCallable, TemplateOverlays};
use crate::stdlib::TestModeRefusal;

/// The manifest whose `foreach` calls the refused `glob()` helper.
const GLOB_MANIFEST: &str = concat!(
    "netsuke_version: \"1.0.0\"\n",
    "targets:\n",
    "  - foreach: glob('glob_files/*.txt')\n",
    "    name: \"{{ item | replace('glob_files/', '') | replace('.txt', '.out') }}\"\n",
    "    command: \"echo {{ item }}\"\n",
);

/// An overlay `glob` drives a `foreach` the surface would otherwise refuse.
#[test]
fn overlay_glob_drives_a_foreach() -> Result<()> {
    ensure!(
        load(GLOB_MANIFEST).is_err(),
        "the probe is vacuous unless `glob()` refuses without the overlay"
    );

    let manifest = load_with(
        GLOB_MANIFEST,
        constant_overlay(
            "glob",
            Value::from_serialize(["glob_files/a.txt", "glob_files/b.txt"]),
        ),
    )?;

    ensure!(
        target_names(&manifest) == ["a.out", "b.out"],
        "the overlay did not drive the expansion: {:?}",
        target_names(&manifest)
    );
    Ok(())
}

/// The manifest whose `when` calls the refused `which()` helper.
const WHICH_MANIFEST: &str = concat!(
    "netsuke_version: \"1.0.0\"\n",
    "targets:\n",
    "  - name: probe\n",
    "    command: \"echo probe\"\n",
    "    when: \"which('probe-tool')\"\n",
);

/// An overlay `which` decides a `when` the surface would otherwise refuse.
#[test]
fn overlay_which_decides_a_when() -> Result<()> {
    ensure!(
        load(WHICH_MANIFEST).is_err(),
        "the probe is vacuous unless `which()` refuses without the overlay"
    );

    let kept = load_with(WHICH_MANIFEST, constant_overlay("which", Value::from(true)))?;
    ensure!(
        target_names(&kept) == ["probe"],
        "a true overlay must keep the target: {:?}",
        target_names(&kept)
    );

    let dropped = load_with(
        WHICH_MANIFEST,
        constant_overlay("which", Value::from(false)),
    )?;
    ensure!(
        target_names(&dropped).is_empty(),
        "a false overlay must filter the target: {:?}",
        target_names(&dropped)
    );
    Ok(())
}

/// The manifest whose command calls the overlay.
const COMMAND_MANIFEST: &str = concat!(
    "netsuke_version: \"1.0.0\"\n",
    "targets:\n",
    "  - name: probe\n",
    "    command: \"echo {{ greet('world') }}\"\n",
);

/// An overlay renders wherever a helper would.
#[test]
fn overlay_renders_in_a_command() -> Result<()> {
    let manifest = load_with(
        COMMAND_MANIFEST,
        TemplateOverlays::default().with_function(
            "greet",
            OverlayCallable::new(|call: &OverlayCall<'_, '_>| {
                let who = call
                    .args()
                    .first()
                    .ok_or_else(|| {
                        MiniJinjaError::new(
                            ErrorKind::InvalidOperation,
                            "the manifest passes one argument",
                        )
                    })?
                    .to_string();
                Ok(Value::from(format!("hello {who}")))
            }),
        ),
    )?;

    ensure!(
        rendered_command(&manifest)? == "echo hello world",
        "the overlay did not render: {}",
        rendered_command(&manifest)?
    );
    Ok(())
}

/// The manifest whose command calls the available `timedelta()` helper.
const TIMEDELTA_MANIFEST: &str = concat!(
    "netsuke_version: \"1.0.0\"\n",
    "targets:\n",
    "  - name: probe\n",
    "    command: \"echo {{ timedelta(days=1) }}\"\n",
);

/// Forwarding to the shadowed helper reproduces the call it displaced.
///
/// The call count is asserted as well as the equality: an overlay that was
/// never dispatched would leave the guard passing on the stdlib's own answer,
/// which is the vacuous form a seeded registration-order fault produces.
#[test]
fn passthrough_overlay_returns_the_shadowed_result() -> Result<()> {
    let shadowed = load(TIMEDELTA_MANIFEST)?;
    let calls = Arc::new(AtomicUsize::new(0));
    let dispatched = Arc::clone(&calls);
    let overlaid = load_with(
        TIMEDELTA_MANIFEST,
        TemplateOverlays::default().with_function(
            "timedelta",
            OverlayCallable::new(move |call: &OverlayCall<'_, '_>| {
                dispatched.fetch_add(1, Ordering::SeqCst);
                let replaced = call.shadowed().ok_or_else(|| {
                    MiniJinjaError::new(ErrorKind::InvalidOperation, "`timedelta` is registered")
                })?;
                replaced.call(call.state(), call.args())
            }),
        ),
    )?;

    ensure!(
        calls.load(Ordering::SeqCst) == 1,
        "the overlay was dispatched {} times, so the comparison is vacuous",
        calls.load(Ordering::SeqCst)
    );
    ensure!(
        rendered_command(&shadowed)? == rendered_command(&overlaid)?,
        "forwarding changed the rendered command: {} vs {}",
        rendered_command(&shadowed)?,
        rendered_command(&overlaid)?
    );
    Ok(())
}

/// The manifest whose command calls the refused `now()` helper.
const NOW_MANIFEST: &str = concat!(
    "netsuke_version: \"1.0.0\"\n",
    "targets:\n",
    "  - name: probe\n",
    "    command: \"echo {{ now() }}\"\n",
);

/// Return the operation named by a test-mode refusal in an error chain.
fn refusal_operation(error: &anyhow::Error) -> Option<&str> {
    error.chain().find_map(|cause| {
        let jinja = cause.downcast_ref::<MiniJinjaError>()?;
        jinja
            .source()?
            .downcast_ref::<TestModeRefusal>()
            .map(TestModeRefusal::operation)
    })
}

/// A refusal stub is handed to its overlay as the shadowed function.
#[test]
fn overlay_now_sees_the_refusing_stub() -> Result<()> {
    ensure!(
        load(NOW_MANIFEST).is_err(),
        "the probe is vacuous unless `now()` refuses under the default clock"
    );

    let calls = Arc::new(AtomicUsize::new(0));
    let dispatched = Arc::clone(&calls);
    let overlays = TemplateOverlays::default().with_function(
        "now",
        OverlayCallable::new(move |call: &OverlayCall<'_, '_>| {
            dispatched.fetch_add(1, Ordering::SeqCst);
            let replaced = call.shadowed().ok_or_else(|| {
                MiniJinjaError::new(ErrorKind::InvalidOperation, "`now` is registered")
            })?;
            replaced.call(call.state(), call.args())
        }),
    );

    let error = load_with(NOW_MANIFEST, overlays)
        .err()
        .context("forwarding to the refusing stub must fail the load")?;
    ensure!(
        calls.load(Ordering::SeqCst) == 1,
        "the overlay was dispatched {} times, so the stack below is the stub's own",
        calls.load(Ordering::SeqCst)
    );
    ensure!(
        refusal_operation(&error) == Some("now"),
        "the forward did not reach the `now` refusal: {error:#}"
    );
    Ok(())
}

/// An empty overlay set is indistinguishable from supplying none.
#[test]
fn default_overlays_equal_no_overlays() -> Result<()> {
    let plain = load(TIMEDELTA_MANIFEST)?;
    let empty = load_with(TIMEDELTA_MANIFEST, TemplateOverlays::default())?;

    ensure!(
        target_names(&plain) == target_names(&empty),
        "the empty overlay set changed the targets: {:?} vs {:?}",
        target_names(&plain),
        target_names(&empty)
    );
    ensure!(
        rendered_command(&plain)? == rendered_command(&empty)?,
        "the empty overlay set changed the rendered command"
    );
    Ok(())
}
