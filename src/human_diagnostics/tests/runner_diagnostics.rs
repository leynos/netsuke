//! Verify runner and contained-report selection in human diagnostics.

use super::*;
use rstest::rstest;
use test_support::{EnLocalizer, en_localizer};

/// Build the runner's own missing-manifest failure with the runner context.
fn runner_failure() -> anyhow::Error {
    let error = runner::RunnerError::ManifestNotFound {
        manifest_name: String::from("Netsukefile"),
        directory: String::from("the current directory"),
        path: std::path::PathBuf::from("/workspace/Netsukefile"),
        message: localization::message(keys::RUNNER_MANIFEST_NOT_FOUND)
            .with_arg("manifest_name", "Netsukefile")
            .with_arg("directory", "the current directory"),
        help: localization::message(keys::RUNNER_MANIFEST_NOT_FOUND_HELP),
    };
    anyhow::Error::new(error).context("Failed to load manifest at Netsukefile.")
}

/// Render the localized summary, code, and help from a contextual runner error.
///
/// Removing the runner-error branch from `select_diagnostic` leaves this chain
/// without a selected diagnostic, so the code, summary, and help assertions
/// fail together.
#[rstest]
fn runner_failures_render_their_own_diagnostic(en_localizer: EnLocalizer) -> Result<()> {
    let _en_localizer = en_localizer;
    let rendered = render_failure(
        &runner_failure(),
        &theme_for(presentation_style(false, true)),
    )
    .context("a runner failure should render a diagnostic")?;
    let flattened = flatten_localized_text(&rendered);
    ensure!(
        flattened.contains("netsuke::runner::manifest_not_found"),
        "the rendered diagnostic must carry the runner variant's stable code: {rendered}"
    );
    ensure!(
        flattened.contains("Manifest 'Netsukefile' not found in the current directory."),
        "the rendered diagnostic must carry its localized summary: {rendered}"
    );
    ensure!(
        flattened.contains("Ensure the manifest exists"),
        "the rendered diagnostic must carry the variant's help text: {rendered}"
    );
    Ok(())
}

/// Render code and help from a `miette::Report` stored in an `anyhow` chain.
///
/// Removing the Report branch from `select_diagnostic` leaves the human path
/// with only the outer context text, so both diagnostic-specific assertions
/// fail.
#[test]
fn contained_reports_render_their_diagnostic() -> Result<()> {
    let diagnostic = MietteDiagnostic::new("the manifest was refused")
        .with_code("netsuke::manifest::schema_version")
        .with_help("declare the supported schema version");
    let report = miette::Report::new(diagnostic);
    let error = anyhow::Error::msg(report.to_string()).context(report);
    let rendered = render_failure(&error, &theme_for(presentation_style(false, true)))
        .context("a contained report should render its diagnostic")?;
    ensure!(
        rendered.contains("netsuke::manifest::schema_version"),
        "the rendered diagnostic must carry the report's stable code: {rendered}"
    );
    ensure!(
        rendered.contains("declare the supported schema version"),
        "the rendered diagnostic must carry the report's help text: {rendered}"
    );
    Ok(())
}

/// A bare runner failure must print its message once, not twice.
///
/// This is the production shape. `handle_runner_error` passes the
/// [`runner::RunnerError`] through without adding a context layer, so the
/// selected diagnostic and the error text are the same sentence, and printing
/// both would repeat it. It lives here rather than beside the manifest cases
/// because it is a runner-diagnostic assertion, and it shares this module's
/// [`flatten_localized_text`](super::flatten_localized_text).
///
/// The assertion is a count, because "the report omits the context line" is
/// vacuously true here: there is no context line to omit, so only counting
/// separates the intended report from the duplicated one. `miette` draws the
/// code first and the message beneath it, so the code preceding the message is
/// also asserted — that ordering is what a `"{err}\n{rendered}"` report breaks.
///
/// Deleting the [`repeats_the_message`] branch leaves every other test in this
/// module passing: they all wrap their error in a context, so their two strings
/// differ and the branch never fires for them.
///
/// It takes the same [`EnLocalizer`] fixture as the neighbouring rendering
/// test because both assert on localized FTL text: the fixture installs the
/// English locale and holds the global localizer lock, so a parallel test that
/// switches locale cannot change the sentence either one searches for.
#[rstest]
fn bare_runner_failures_print_their_message_once(en_localizer: EnLocalizer) -> Result<()> {
    let _en_localizer = en_localizer;
    let error = anyhow::Error::new(runner::RunnerError::ManifestNotFound {
        manifest_name: String::from("Netsukefile"),
        directory: String::from("the current directory"),
        path: std::path::PathBuf::from("/workspace/Netsukefile"),
        message: localization::message(keys::RUNNER_MANIFEST_NOT_FOUND)
            .with_arg("manifest_name", "Netsukefile")
            .with_arg("directory", "the current directory"),
        help: localization::message(keys::RUNNER_MANIFEST_NOT_FOUND_HELP),
    });
    let report = failure_report(&error, &theme_for(presentation_style(false, true)))
        .context("a bare runner failure should report")?;
    let flattened = flatten_localized_text(&report);
    ensure!(
        flattened
            .matches("Manifest 'Netsukefile' not found")
            .count()
            == 1,
        "a bare runner failure must print its message exactly once: {flattened}"
    );
    let code = flattened
        .find("netsuke::runner::manifest_not_found")
        .context("the report must carry the runner variant's stable code")?;
    let summary = flattened
        .find("Manifest 'Netsukefile' not found")
        .context("the report must carry the localized summary")?;
    ensure!(
        code < summary,
        "the code must precede the summary rather than a duplicate line: {flattened}"
    );
    Ok(())
}

/// Prefer a runner error when one chain also contains a manifest error.
///
/// Reversing the runner and manifest checks in `select_diagnostic` makes the
/// manifest diagnostic win. The stable runner code assertion detects that
/// branch-order regression.
#[test]
fn runner_failures_outrank_manifest_failures_in_one_chain() -> Result<()> {
    let runner_error = runner::RunnerError::ManifestNotFound {
        manifest_name: String::from("Netsukefile"),
        directory: String::from("the current directory"),
        path: std::path::PathBuf::from("/workspace/Netsukefile"),
        message: localization::message(keys::RUNNER_MANIFEST_NOT_FOUND)
            .with_arg("manifest_name", "Netsukefile")
            .with_arg("directory", "the current directory"),
        help: localization::message(keys::RUNNER_MANIFEST_NOT_FOUND_HELP),
    };
    let manifest_error = manifest::ManifestError::Parse {
        source: Box::new(
            MietteDiagnostic::new("declares an unsupported schema")
                .with_code("netsuke::manifest::schema_version"),
        ),
        message: localization::message(keys::MANIFEST_PARSE),
    };
    let error = anyhow::Error::new(manifest_error).context(runner_error);
    let rendered = render_failure(&error, &theme_for(presentation_style(false, true)))
        .context("a chain carrying both candidates should still render")?;
    ensure!(
        rendered.contains("netsuke::runner::manifest_not_found"),
        "the runner failure must win the selection: {rendered}"
    );
    ensure!(
        !rendered.contains("declares an unsupported schema"),
        "the manifest diagnostic must not replace the runner diagnostic: {rendered}"
    );
    Ok(())
}
