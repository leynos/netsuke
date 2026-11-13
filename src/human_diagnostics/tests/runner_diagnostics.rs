//! Verify runner and contained-report selection in human diagnostics.

use super::*;

/// Remove bidi isolates and terminal wrapping from localized output.
fn flatten_localized_text(rendered: &str) -> String {
    let without_bidi_isolates = rendered
        .chars()
        .filter(|character| !matches!(*character, '\u{2068}' | '\u{2069}'))
        .collect::<String>();
    without_bidi_isolates
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

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
#[test]
fn runner_failures_render_their_own_diagnostic() -> Result<()> {
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
