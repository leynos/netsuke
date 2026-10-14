//! What each query-refused helper says when a manifest calls it.
//!
//! Manifest discovery renders `netsuke help targets`, which must not disclose
//! host state or perform side effects. Every helper that is legitimate at build
//! time but unsafe while discovering is registered as a stub that fails with a
//! recognizable marker, and those messages are user-visible text: they reach
//! the diagnostic a user sees when a manifest overreaches. This module pins
//! them one row per helper, so a later refactor that reorganizes the stubs
//! cannot quietly reword a refusal.
//!
//! # Why one document rather than one snapshot per helper
//!
//! The build surface's characterization groups every fixture into one document
//! per entry point, and this follows it for the same reason. Messages that
//! differ only in their leading word are read *comparatively*: a reviewer
//! checking that every name is spelled as the helper registers it needs the
//! rows side by side, which one file gives and one file per helper does not.
//! Every row also shares the marker tail, so a rewording anywhere changes this
//! one file — the per-helper split would buy no extra visibility for the
//! change that actually matters.
//!
//! The rows are labelled by helper name and call shape, so a reader who greps
//! for one helper lands on its block.
//!
//! The settings drop insta's module prefix, so EP-M1 can move this file into
//! `src/stdlib/restricted_helpers/tests.rs` without renaming the snapshot — a
//! rename during a pure move would read as a behaviour change.
//!
//! # What is asserted
//!
//! Each row renders a one-line template that calls its helper and contributes
//! the resulting error to a single snapshot document. The template is rendered
//! through [`register_manifest_query`], the same registration `netsuke help
//! targets` installs, so the row observes the real query surface rather than a
//! reconstruction of it.
//!
//! A refusal must also be *recognizable*, which is a separate claim from its
//! text: `is_manifest_query_disabled_error` decides whether the classifier
//! treats a `when` failure as a conditional rather than a load failure. Every
//! row asserts recognition too, because a helper registered by the *real*
//! implementation instead of a stub would render successfully, and the text
//! assertion alone would report only a missing error.

use super::{is_manifest_query_disabled_error, register_manifest_query};
use anyhow::Result;
use minijinja::Environment;
use rstest::rstest;

/// One refusing helper: the name its refusal must lead with, and a template
/// that calls it.
///
/// The two are kept as separate fields rather than one derived from the other.
/// A filter and a function can share a name (`which` is both), so the template
/// is the only thing that says which registration a row exercises, and the name
/// is the only thing that says what the refusal must lead with. Deriving either
/// from the other would silence exactly the mismatch worth catching.
struct Refusal {
    /// The helper name the refusal text must begin with.
    name: &'static str,
    /// The template that calls the helper.
    template: &'static str,
}

/// The refusing inventory.
///
/// The templates are written to match the widest call shape each helper's real
/// counterpart accepts, so a stub that stopped being registered as a stub would
/// not pass merely by rejecting the arguments. `shell` and `grep` are filters
/// and need a value on the left, which the empty string supplies.
///
/// `which` appears twice, once as a filter and once as a function. Both
/// registrations are separate calls in the implementation, so a change that
/// dropped one would leave the other green.
///
/// Every row here is a helper the surface *refuses*, which is a different set
/// from the helpers `FILE_TESTS` names. Completeness is the requirement — a
/// refusal with no row is a message that can be reworded unobserved — so when
/// adding a stub to `query_helpers.rs`, add its row here too. This list drifted
/// once already: `glob` was registered but unrowed until it was found by
/// enumerating the registrations against this table.
const REFUSALS: &[Refusal] = &[
    Refusal {
        name: "env",
        template: "{{ env('PATH') }}",
    },
    Refusal {
        name: "glob",
        template: "{{ glob('glob_files/*.txt') }}",
    },
    Refusal {
        name: "fetch",
        template: "{{ fetch('https://example.invalid/toolchain.json') }}",
    },
    Refusal {
        name: "shell",
        template: "{{ '' | shell('echo fixture') }}",
    },
    Refusal {
        name: "grep",
        template: "{{ 'x' | grep('x') }}",
    },
    Refusal {
        name: "contents",
        template: "{{ 'Cargo.toml' | contents }}",
    },
    Refusal {
        name: "which",
        template: "{{ 'cargo' | which }}",
    },
    Refusal {
        name: "which",
        template: "{{ which('cargo') }}",
    },
    Refusal {
        name: "command_available",
        template: "{{ command_available('cargo') }}",
    },
    Refusal {
        name: "now",
        template: "{{ now() }}",
    },
    Refusal {
        name: "realpath",
        template: "{{ '.' | realpath }}",
    },
    Refusal {
        name: "expanduser",
        template: "{{ '~' | expanduser }}",
    },
    Refusal {
        name: "size",
        template: "{{ 'Cargo.toml' | size }}",
    },
    Refusal {
        name: "linecount",
        template: "{{ 'Cargo.toml' | linecount }}",
    },
    Refusal {
        name: "hash",
        template: "{{ 'Cargo.toml' | hash }}",
    },
    Refusal {
        name: "digest",
        template: "{{ 'Cargo.toml' | digest }}",
    },
    Refusal {
        name: "dir",
        template: "{{ 'Cargo.toml' is dir }}",
    },
    Refusal {
        name: "file",
        template: "{{ 'Cargo.toml' is file }}",
    },
    Refusal {
        name: "symlink",
        template: "{{ 'Cargo.toml' is symlink }}",
    },
    Refusal {
        name: "pipe",
        template: "{{ 'Cargo.toml' is pipe }}",
    },
    Refusal {
        name: "block_device",
        template: "{{ 'Cargo.toml' is block_device }}",
    },
    Refusal {
        name: "char_device",
        template: "{{ 'Cargo.toml' is char_device }}",
    },
    Refusal {
        name: "device",
        template: "{{ 'Cargo.toml' is device }}",
    },
];

/// Snapshot every query refusal the restricted surface produces.
///
/// One test rather than one per helper: each helper is already a row of
/// [`REFUSALS`], and splitting it into twenty-odd test functions would move the
/// table's shape into the test harness without making any single refusal easier
/// to read. The document preserves the table's order for the same reason.
///
/// Every row runs even when an earlier one fails, so one broken helper does not
/// hide the state of the rest. The failures are collected and reported together
/// for the same reason, and they are reported *instead of* the snapshot: a row
/// that produced no text would otherwise show up only as a missing block in a
/// document diff, which reads as an edit rather than as a broken helper.
#[rstest]
fn refused_helpers_report_their_own_name() -> Result<()> {
    let mut failures = Vec::new();
    let mut blocks = Vec::new();
    for (index, refusal) in REFUSALS.iter().enumerate() {
        match check_refusal(refusal) {
            Ok(text) => blocks.push(format!(
                "=== {index:02} {}: {} ===\n{text}",
                refusal.name, refusal.template
            )),
            Err(error) => failures.push(format!("{}: {error}", refusal.name)),
        }
    }
    anyhow::ensure!(
        failures.is_empty(),
        "{} of {} query-refusal rows failed:\n{}",
        failures.len(),
        REFUSALS.len(),
        failures.join("\n")
    );
    let document = blocks.join("\n\n");
    crate::snapshot_test_support::stable_snapshot_settings("manifest_load_options").bind(|| {
        insta::assert_snapshot!("refusals", document);
    });
    Ok(())
}

/// Assert one helper is refused, is recognizable, and names itself.
///
/// Returns the refusal text so the caller can place it in the document. The
/// text is owned rather than borrowed because it comes from the rendered
/// error's detail, which the error owns and drops on return.
///
/// # Errors
///
/// Returns an error when the template renders, when the error is not a query
/// refusal, or when the refusal leads with a different helper's name.
fn check_refusal(refusal: &Refusal) -> Result<String> {
    let error = refusal_from(refusal.template)?;
    // The message rather than the rendered error: `Display` prepends the error
    // kind and appends the template position, neither of which the stub
    // controls, and both of which would blur a one-line row. Requiring the
    // message is a claim in its own right — a stub that raised a detail-less
    // error would render the kind alone, which is indistinguishable from
    // MiniJinja's own failure and so is worth failing on.
    let text = error
        .detail()
        .ok_or_else(|| anyhow::anyhow!("the refusal carries no message: {error}"))?;
    // The name is asserted before the snapshot, because a stub built from the
    // wrong table row would still carry the marker — the document would match
    // while the diagnostic named another helper.
    anyhow::ensure!(
        text.starts_with(refusal.name),
        "the refusal names another helper: {text}"
    );
    Ok(text.to_owned())
}

/// Render `template` through the query surface and return the error.
///
/// # Errors
///
/// Returns an error when the template renders successfully, which means the
/// helper named in it is no longer refused, or when rendering fails with an
/// error that is not a query refusal. Both are failures of the row's premise
/// rather than of the assertion it makes.
fn refusal_from(template: &str) -> Result<minijinja::Error> {
    let mut env = Environment::new();
    register_manifest_query(&mut env);
    match env.render_str(template, ()) {
        Ok(rendered) => anyhow::bail!(
            "the query surface rendered {template:?} successfully as {rendered:?}; \
             the helper it calls is no longer refused"
        ),
        Err(error) => {
            anyhow::ensure!(
                is_manifest_query_disabled_error(&error),
                "the query surface refused {template:?} with an error that is not a \
                 query refusal: {error}"
            );
            Ok(error)
        }
    }
}
