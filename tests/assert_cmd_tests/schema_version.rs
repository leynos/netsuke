//! End-to-end CLI coverage for the manifest schema-version gate.
//!
//! The cases live beside the general CLI tests so neither file dilutes the
//! other's function mix. Each drives the real binary, so the assertions bind
//! the process boundary rather than the loader alone.

use super::create_netsuke_command;
use anyhow::{Context, Result, ensure};
use rstest::{fixture, rstest};
use std::fs;
use std::path::Path;
use tempfile::{TempDir, tempdir};

/// Write the minimal hello manifest declaring `version` as its schema.
fn write_schema_manifest(directory: &Path, version: &str) -> Result<()> {
    fs::write(
        directory.join("Netsukefile"),
        format!(
            concat!(
                "netsuke_version: '{version}'\n",
                "targets:\n",
                "  - name: hello\n",
                "    command: echo hello\n",
            ),
            version = version
        ),
    )
    .context("write schema-version manifest")
}

/// Create a workspace whose manifest declares an unsupported schema version.
#[fixture]
fn unsupported_schema_workspace() -> Result<TempDir> {
    let temp = tempdir().context("create schema-version workspace")?;
    write_schema_manifest(temp.path(), "1.1.0")?;
    Ok(temp)
}

/// Assert a JSON-mode rejection carries the parse code and the schema cause.
fn assert_schema_rejection_json(stderr: &[u8], command: &str) -> Result<()> {
    let rendered = String::from_utf8_lossy(stderr);
    let diagnostics: serde_json::Value = serde_json::from_str(&rendered)
        .with_context(|| format!("parse {command} diagnostics JSON"))?;
    let diagnostic = diagnostics
        .pointer("/diagnostics/0")
        .with_context(|| format!("{command} should report a diagnostic"))?;
    ensure!(
        diagnostic.get("code").and_then(serde_json::Value::as_str)
            == Some("netsuke::manifest::parse"),
        "{command} should preserve the parse diagnostic code: {diagnostic}"
    );
    let causes = diagnostic
        .get("causes")
        .and_then(serde_json::Value::as_array)
        .with_context(|| format!("{command} diagnostic should include causes"))?;
    ensure!(
        causes.iter().any(|cause| cause
            .as_str()
            .is_some_and(|text| text.contains("netsuke::manifest::schema_version"))),
        "{command} should expose the schema diagnostic code: {causes:?}"
    );
    Ok(())
}

#[rstest]
fn generate_rejects_unsupported_schema_before_creating_output(
    unsupported_schema_workspace: Result<TempDir>,
) -> Result<()> {
    let temp = unsupported_schema_workspace?;
    let output_path = temp.path().join("out.ninja");

    let unsupported = create_netsuke_command(temp.path())
        .args(["--json", "generate", "--output", "out.ninja"])
        .output()
        .context("run generate with an unsupported schema version")?;
    ensure!(
        !unsupported.status.success(),
        "generate must reject schema version 1.1.0"
    );
    ensure!(
        !output_path.exists(),
        "generate must not create output for a rejected manifest"
    );
    assert_schema_rejection_json(&unsupported.stderr, "generate")?;

    write_schema_manifest(temp.path(), "1.0.0")?;
    create_netsuke_command(temp.path())
        .args(["generate", "--output", "out.ninja"])
        .assert()
        .success();
    ensure!(
        output_path.exists(),
        "the paired supported manifest should generate output"
    );
    Ok(())
}

#[rstest]
fn human_output_renders_the_schema_version_rejection(
    unsupported_schema_workspace: Result<TempDir>,
) -> Result<()> {
    let temp = unsupported_schema_workspace?;

    let output = create_netsuke_command(temp.path())
        .args(["generate", "--output", "out.ninja"])
        .output()
        .context("run generate without JSON diagnostics")?;
    ensure!(
        !output.status.success(),
        "generate must reject schema version 1.1.0 on the human path too"
    );
    ensure!(
        !temp.path().join("out.ninja").exists(),
        "generate must not create output for a rejected manifest"
    );
    let stderr = String::from_utf8(output.stderr).context("human diagnostics should be UTF-8")?;
    ensure!(
        stderr.contains("netsuke::manifest::schema_version"),
        "human output must name the schema-version diagnostic code: {stderr}"
    );
    ensure!(
        stderr.contains("1.0"),
        "human output must name the schema this Netsuke supports: {stderr}"
    );
    Ok(())
}

#[cfg(feature = "lint")]
#[rstest]
fn check_rejects_unsupported_schema_and_accepts_supported_schema(
    unsupported_schema_workspace: Result<TempDir>,
) -> Result<()> {
    let temp = unsupported_schema_workspace?;
    let unsupported = create_netsuke_command(temp.path())
        .args(["--json", "check"])
        .output()
        .context("run check with an unsupported schema version")?;
    ensure!(
        !unsupported.status.success(),
        "check must reject schema version 1.1.0"
    );
    assert_schema_rejection_json(&unsupported.stderr, "check")?;

    write_schema_manifest(temp.path(), "1.0.0")?;
    create_netsuke_command(temp.path())
        .args(["check"])
        .assert()
        .success();
    Ok(())
}

/// Write a manifest whose schema declaration is `declaration`, verbatim.
///
/// The value is spliced into the YAML unquoted so a case can supply `7`, a
/// non-string scalar, as readily as `'1.0.3'`. `declaration` is `None` for the
/// missing case: an omitted key is the only shape that reaches
/// `SchemaVersionRejection::Missing`, because a blank value is a null document
/// root and non-mapping roots are admitted. Every other field is a valid
/// manifest, so the only fault the binary can report is the declaration.
fn write_raw_declaration(directory: &Path, declaration: Option<&str>) -> Result<()> {
    let declaration_line =
        declaration.map_or_else(String::new, |value| format!("netsuke_version: {value}\n"));
    fs::write(
        directory.join("Netsukefile"),
        format!(
            concat!(
                "{declaration_line}",
                "targets:\n",
                "  - name: hello\n",
                "    command: echo hello\n",
            ),
            declaration_line = declaration_line
        ),
    )
    .context("write raw schema declaration manifest")
}

/// The summary fragment each rejection class must render, and nothing else.
///
/// A case is identified by the phrase only its own message carries, because a
/// substring common to several classes cannot show that the right class was
/// chosen. `MAJOR.MINOR.0` alone would not do it: it appears in the remedy for
/// every class *and* in the `invalid` summary, so an assertion built on it
/// would still pass if `missing` and `invalid` were swapped.
#[derive(Clone, Copy)]
enum RejectionSummary {
    /// Only `manifest.schema_version.missing` carries this phrase.
    Missing,
    /// `not_string`, `invalid` and `not_canonical` share the invalid summary.
    Invalid,
    /// The only summary naming the declared and supported schema.
    Unsupported,
}

impl RejectionSummary {
    /// Return the phrase unique to this class's summary line.
    const fn phrase(self) -> &'static str {
        match self {
            Self::Missing => "must declare 'netsuke_version'",
            Self::Invalid => "must be a semantic-version release",
            Self::Unsupported => "but this Netsuke supports schema",
        }
    }
}

/// Assert a human-mode rejection renders its own summary and the shared remedy.
///
/// The summary and the remedy are asserted separately on purpose. Every class
/// carries `manifest.schema_version.help`, and that text is the only place a
/// class-specific summary is *not* required — while `MAJOR.MINOR.0` appears in
/// both the help and the `invalid` summary, so asserting the remedy alone
/// cannot distinguish them. Requiring the class phrase is what makes the case
/// discriminating.
fn assert_human_rejection_names_code_and_remedy(
    stderr: &str,
    case: &str,
    summary: RejectionSummary,
) -> Result<()> {
    ensure!(
        stderr.contains("netsuke::manifest::schema_version"),
        "{case}: human output must name the schema-version diagnostic code: {stderr}"
    );
    // `miette` wraps the graphical layout to the terminal width, so a phrase
    // spanning a line break is still the phrase. Collapsing whitespace before
    // matching keeps the assertion about the text rather than the wrapping.
    let flattened = stderr.split_whitespace().collect::<Vec<_>>().join(" ");
    ensure!(
        flattened.contains(summary.phrase()),
        "{case}: human output must render this class's own summary ({}): {stderr}",
        summary.phrase()
    );
    ensure!(
        flattened.contains("MAJOR.MINOR.0"),
        "{case}: human output must state the MAJOR.MINOR.0 remedy: {stderr}"
    );
    Ok(())
}

/// Every rejection class must reach the operator as a rendered diagnosis.
///
/// The predicate is unit-tested elsewhere; what this covers is the process
/// boundary — that each class survives the loader, the diagnostic mapping, and
/// the human renderer with its own summary, its code, and the shared remedy
/// intact. The four classes take different paths through `admit_document`, so a
/// regression in any single arm would otherwise be invisible.
#[rstest]
#[case::missing(None, "a missing declaration", RejectionSummary::Missing)]
#[case::not_string(Some("7"), "a non-string declaration", RejectionSummary::Invalid)]
#[case::invalid(
    Some("'not-semver'"),
    "a malformed declaration",
    RejectionSummary::Invalid
)]
#[case::non_canonical(
    Some("'1.0.3'"),
    "a non-canonical declaration",
    RejectionSummary::Invalid
)]
#[case::unsupported(
    Some("'1.1.0'"),
    "an unsupported declaration",
    RejectionSummary::Unsupported
)]
fn every_rejection_class_renders_a_diagnosis(
    #[case] declaration: Option<&str>,
    #[case] case: &str,
    #[case] summary: RejectionSummary,
) -> Result<()> {
    let temp = tempdir().context("create rejection-class workspace")?;
    write_raw_declaration(temp.path(), declaration)?;
    let output = create_netsuke_command(temp.path())
        .args(["generate", "--output", "out.ninja"])
        .output()
        .with_context(|| format!("run generate with {case}"))?;
    ensure!(
        !output.status.success(),
        "{case}: generate must fail rather than build"
    );
    ensure!(
        !temp.path().join("out.ninja").exists(),
        "{case}: a rejected manifest must not produce output"
    );
    let stderr =
        String::from_utf8(output.stderr).with_context(|| format!("{case}: stderr is UTF-8"))?;
    assert_human_rejection_names_code_and_remedy(&stderr, case, summary)
}
