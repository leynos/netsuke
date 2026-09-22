//! The general source-excerpt guard over rendered diagnostic documents.
//!
//! A cause is serialized from `Display`, so whatever a dependency's formatter
//! chooses to render reaches a field the schema documents as a compact reason.
//! `serde-saphyr` 1.2.0 made that concrete by annotating its parse errors with a
//! source excerpt. The manifest boundary normalizes that one case away, but the
//! hazard is general: any dependency rendering an excerpt can reintroduce it,
//! and only a guard that walks every diagnostic does anything about the next one.
//!
//! These cases live here rather than in the parent so the structural tests stay
//! within the repository's 400-line cap; the parent reaches them by declaring
//! this module. The `related` recursion is not decoration — the serializer
//! renders a related diagnostic as a full entry of the same shape, so a guard
//! that reads only the top-level entry leaves the nested cause chains unguarded.

use super::super::circular_dependency_error;
use super::super::{render_diagnostic_json, render_error_json};
use super::first_diagnostic;
use crate::localization::{self, keys};
use crate::manifest;
use anyhow::{Context, Result, ensure};
use miette::Diagnostic;
use rstest::rstest;
use serde_json::{Map, Value};
use std::error::Error as StdError;
use std::fmt;
use test_support::{EnLocalizer, en_localizer};

/// Markers an upstream formatter emits when it annotates an error with a
/// source excerpt.
///
/// Causes are rendered from `Display`, so an upstream presentation change can
/// leak an excerpt into a field meant to carry a compact reason. Netsuke
/// reports the failing location through its own `source` and `labels` fields,
/// so an excerpt in `causes` is a leak whatever produced it.
///
/// Neither marker occurs in the localized catalogues or in Netsuke's own
/// diagnostic prose, so a match means a dependency really embedded an excerpt.
const SNIPPET_MARKERS: [&str; 2] = ["\n -->", "<input>"];

/// Visits every cause in a diagnostic object and in its nested `related` tree.
///
/// A related diagnostic serializes as a full entry of the same shape, cause
/// chain included, so the walk has to descend rather than read the top level
/// alone. A missing `related` is treated as empty: the schema always writes the
/// field, but requiring it here would add a schema assertion to a contract that
/// is only about excerpts.
fn visit_causes(
    diagnostic: &Map<String, Value>,
    visit: &mut impl FnMut(&str) -> Result<()>,
) -> Result<()> {
    let causes = diagnostic
        .get("causes")
        .and_then(Value::as_array)
        .context("causes should be present")?;
    for cause in causes {
        visit(
            cause
                .as_str()
                .context("each cause should be a JSON string")?,
        )?;
    }
    let related = diagnostic.get("related").and_then(Value::as_array);
    for entry in related.into_iter().flatten() {
        let nested = entry
            .as_object()
            .context("each related entry should be an object")?;
        visit_causes(nested, visit)?;
    }
    Ok(())
}

/// Asserts that no cause in a rendered document carries a source excerpt.
///
/// Keyed on the annotation markers rather than on line count: an upstream error
/// may legitimately span multiple lines for reasons unrelated to source
/// excerpts, and rejecting those would fail on a factor this contract does not
/// care about.
fn ensure_causes_free_of_source_excerpts(document: &str) -> Result<()> {
    let value = serde_json::from_str::<Value>(document)?;
    let diagnostic = first_diagnostic(&value)?;
    let mut examined = 0_usize;
    visit_causes(diagnostic, &mut |cause| {
        examined += 1;
        for marker in SNIPPET_MARKERS {
            ensure!(
                !cause.contains(marker),
                "cause leaked a source excerpt ({marker:?}): {cause}"
            );
        }
        Ok(())
    })?;
    ensure!(
        examined > 0,
        "a parsed document should record at least one cause"
    );
    Ok(())
}

/// No rendered cause may carry an upstream source excerpt, whichever dependency
/// produced it.
///
/// `serde-saphyr` 1.2.0 began rendering parse errors with an annotated snippet,
/// which reached `causes` because a cause is serialized from `Display`. The
/// manifest boundary normalizes that one away, but the hazard is general: any
/// dependency rendering a source excerpt can reintroduce it. This pins the
/// contract across both cause-collection paths, over the YAML and structural
/// parse dependencies and a Netsuke-owned chain.
#[rstest]
#[case::yaml_parse("targets:\n\t- name: test\n")]
#[case::structural_parse("")]
fn rendered_causes_never_carry_source_excerpts(
    #[case] yaml: &str,
    en_localizer: EnLocalizer,
) -> Result<()> {
    let _en_localizer = en_localizer;
    let err = manifest::from_str(yaml).expect_err("invalid manifest should fail to parse");
    let manifest_err = err
        .downcast_ref::<manifest::ManifestError>()
        .context("expected ManifestError")?;
    ensure_causes_free_of_source_excerpts(&render_diagnostic_json(manifest_err)?)
}

/// The plain-error path collects causes from a standard-error chain rather than
/// from a diagnostic source, so it needs the contract pinned separately.
///
/// The fixture is a Netsuke-owned error, so this case pins that the plain path
/// is *wired into* the guard and that its own rendering is clean. It cannot
/// demonstrate the guard's sensitivity to an excerpt: a marker in the fixture
/// would fail the assertion by construction rather than by a leak.
/// `ensure_causes_free_of_source_excerpts_rejects_a_leaked_excerpt` carries that
/// burden instead.
#[rstest]
fn rendered_plain_error_causes_never_carry_source_excerpts(
    en_localizer: EnLocalizer,
) -> Result<()> {
    let _en_localizer = en_localizer;
    let error = anyhow::Error::new(circular_dependency_error())
        .context(localization::message(keys::RUNNER_CONTEXT_BUILD_GRAPH));
    ensure_causes_free_of_source_excerpts(&render_error_json(error.as_ref())?)
}

/// Builds a one-diagnostic document whose causes are supplied by the caller, so
/// a case can plant a marker at a chosen depth.
fn document_with_causes(top_level: &[&str], related_level: &[&str]) -> String {
    let nested = serde_json::json!({
        "message": "related",
        "code": null,
        "severity": "error",
        "help": null,
        "url": null,
        "causes": related_level,
        "source": null,
        "primary_span": null,
        "labels": [],
        "related": [],
    });
    serde_json::json!({
        "schema_version": 1,
        "generator": { "name": "netsuke", "version": "0.0.0" },
        "diagnostics": [{
            "message": "top level",
            "code": null,
            "severity": "error",
            "help": null,
            "url": null,
            "causes": top_level,
            "source": null,
            "primary_span": null,
            "labels": [],
            "related": [nested],
        }],
    })
    .to_string()
}

/// The guard must be able to fail, and must name the leak when it does.
///
/// Without this case a green suite says only that the helper agreed with the
/// current dependencies. It would not distinguish a working guard from one that
/// inspects nothing — so this plants the very excerpt the guard exists to catch.
#[rstest]
fn ensure_causes_free_of_source_excerpts_rejects_a_leaked_excerpt() -> Result<()> {
    let document = document_with_causes(&["parse failed\n --> <input>:1:1"], &[]);
    let err = ensure_causes_free_of_source_excerpts(&document)
        .expect_err("a marker-bearing cause must be rejected");
    ensure!(
        err.to_string().contains("leaked a source excerpt"),
        "the failure should name the leak: {err}"
    );
    Ok(())
}

/// A leak nested in a related diagnostic is still caught.
///
/// The serializer renders `related` entries as full diagnostics with their own
/// cause chains, so a guard reading only the top-level entry would pass this
/// document. That is the regression this case pins.
#[rstest]
fn ensure_causes_free_of_source_excerpts_rejects_a_related_leak() -> Result<()> {
    let document = document_with_causes(
        &["tabs disallowed within this context"],
        &["\n --> <input>"],
    );
    let err = ensure_causes_free_of_source_excerpts(&document)
        .expect_err("a marker-bearing related cause must be rejected");
    ensure!(
        err.to_string().contains("leaked a source excerpt"),
        "the failure should name the leak: {err}"
    );
    Ok(())
}

/// The excerpt-bearing source a related diagnostic carries as its cause.
#[derive(Debug)]
struct SnippetSource;

impl fmt::Display for SnippetSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("3 | let x = 1;\n --> <input>:3:1")
    }
}

impl StdError for SnippetSource {}

/// A related diagnostic whose own cause chain carries the excerpt.
#[derive(Debug)]
struct RelatedNetsukeError {
    /// The leaking cause, reached through `Error::source`.
    cause: SnippetSource,
}

impl RelatedNetsukeError {
    /// Builds the fixture with its leaking cause already attached.
    fn new() -> Self {
        Self {
            cause: SnippetSource,
        }
    }
}

impl fmt::Display for RelatedNetsukeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("related diagnostic")
    }
}

impl StdError for RelatedNetsukeError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(&self.cause)
    }
}

impl Diagnostic for RelatedNetsukeError {}

/// An outer diagnostic that reports one related diagnostic.
#[derive(Debug)]
struct OuterNetsukeError {
    /// The related diagnostic the document must descend into.
    related: RelatedNetsukeError,
}

impl fmt::Display for OuterNetsukeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("outer diagnostic")
    }
}

impl StdError for OuterNetsukeError {}

impl Diagnostic for OuterNetsukeError {
    fn related(&self) -> Option<Box<dyn Iterator<Item = &dyn Diagnostic> + '_>> {
        Some(Box::new(std::iter::once(&self.related as &dyn Diagnostic)))
    }
}

/// The recursion guards a shape the serializer really produces.
///
/// The cases above build their documents by hand, so they would pass even if the
/// serializer never emitted a `related` entry with a cause chain — the recursion
/// would be dead code guarding a field shape that does not occur. This case
/// routes a leak through `render_diagnostic_json` itself, which makes the
/// serializer's own output the evidence rather than an assumed schema.
#[rstest]
fn a_leak_reached_through_the_serializer_is_caught(en_localizer: EnLocalizer) -> Result<()> {
    let _en_localizer = en_localizer;
    let error = OuterNetsukeError {
        related: RelatedNetsukeError::new(),
    };
    let document = render_diagnostic_json(&error)?;
    let value = serde_json::from_str::<Value>(&document)?;
    let related = first_diagnostic(&value)?
        .get("related")
        .and_then(Value::as_array)
        .context("the serializer should render a related entry")?;
    ensure!(
        !related.is_empty(),
        "the serializer should render the related diagnostic, not drop it"
    );
    let err = ensure_causes_free_of_source_excerpts(&document)
        .expect_err("the serializer's own related leak must be rejected");
    ensure!(
        err.to_string().contains("leaked a source excerpt"),
        "the failure should name the leak: {err}"
    );
    Ok(())
}
