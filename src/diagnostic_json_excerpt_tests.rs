//! The source-excerpt guard over rendered diagnostic documents.
//!
//! A cause is serialized from an error's `Display`, so a dependency's formatter
//! decides what reaches a field the schema documents as the error-cause chain.
//! `serde-saphyr` 1.2.0 made that concrete by annotating its parse errors with a
//! source excerpt. The manifest boundary normalizes that one case away: Netsuke
//! reports the failing location through its own `source` and `labels` fields, so
//! a duplicated excerpt in `causes` adds nothing. That is a deliberate
//! normalization at a miette-diagnostic boundary, not a general rewriting rule —
//! see `rendered_plain_error_causes_never_carry_source_excerpts` for why the
//! plain path cannot copy it.
//!
//! If the guard fires on a plain-chain cause, the answer is not a general text
//! filter: that text is the error's only location report, so dropping it would
//! make the field say less than the error knows. The fix is to give the failing
//! error a rendered diagnostic at the boundary Netsuke owns, the way the manifest
//! parser already does, and let the diagnostic carry `source` and `labels`.
//!
//! These cases live here rather than in the parent so the structural tests stay
//! within the repository's 400-line cap; the parent declares this module. The
//! `related` recursion is not decoration — the serializer renders a related
//! diagnostic as a full entry of the same shape, so a guard reading only the
//! top-level entry would leave the nested cause chains unguarded.

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
/// add a rendered snippet to a field the schema documents as the error-cause
/// chain. Netsuke reports a miette diagnostic's failing location through its own
/// `source` and `labels` fields, so a duplicated excerpt there is noise.
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
/// Keyed on the annotation markers rather than on line count: an error may
/// legitimately span multiple lines for reasons unrelated to source excerpts,
/// and rejecting those would fail on a factor this contract does not care about.
/// Multi-line causes are expected, not tolerated — `serde-saphyr` renders a
/// plain multi-line parse reason on the YAML path today.
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

/// Asserts the guard rejects a document, and that its failure names the leak.
///
/// The two rejection cases plant their excerpt at different depths, but they
/// agree on what a rejection must look like. Sharing that assertion keeps a
/// change to the failure message in one place instead of two.
fn assert_rejected_for_leaking(document: &str, expected: &str) -> Result<()> {
    let err = ensure_causes_free_of_source_excerpts(document).expect_err(expected);
    ensure!(
        err.to_string().contains("leaked a source excerpt"),
        "the failure should name the leak: {err}"
    );
    Ok(())
}

/// A miette diagnostic's rendered causes may not carry an upstream excerpt.
///
/// `serde-saphyr` 1.2.0 began rendering parse errors with an annotated snippet,
/// which reached `causes` because a cause is serialized from `Display`. The
/// manifest boundary normalizes that one away with a miette diagnostic that
/// carries the failing location in its `source` and `labels` fields instead.
/// That normalization is what this case holds to account: it pins the contract
/// over the YAML and structural parse dependencies, so a dependency re-rendering
/// an excerpt would fail here.
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
/// The fixture is a Netsuke-owned error: its chain carries only Netsuke's own
/// circular-dependency message. That is the scope of this contract on this path.
/// `causes` is documented as the error-cause chain itself — the only location
/// channel a plain error has, because `source`, `primary_span`, and `labels` are
/// all empty here — so an upstream error that renders a source excerpt is
/// reporting the only location it will ever report. Netsuke therefore passes
/// that text through rather than substituting a shorter reason, and this case
/// pins that Netsuke's own causes stay free of excerpts.
/// `ensure_causes_free_of_source_excerpts_rejects_a_leaked_excerpt` carries the
/// burden of showing the guard can fail.
#[rstest]
fn rendered_plain_error_causes_never_carry_source_excerpts(
    en_localizer: EnLocalizer,
) -> Result<()> {
    let _en_localizer = en_localizer;
    let error = anyhow::Error::new(circular_dependency_error())
        .context(localization::message(keys::RUNNER_CONTEXT_BUILD_GRAPH));
    ensure_causes_free_of_source_excerpts(&render_error_json(error.as_ref())?)
}

/// The guard must be able to fail, and must name the leak when it does.
///
/// Without a case like this a green suite says only that the helper agreed with
/// the current dependencies; it would not distinguish a working guard from one
/// that inspects nothing. The document is built by the serializer rather than by
/// hand so the leak the guard is asked to catch is one the serializer really
/// emits — `render_error_json` copies a source's `Display` into `causes`, and
/// this fixture's source carries the excerpt.
#[rstest]
fn ensure_causes_free_of_source_excerpts_rejects_a_leaked_excerpt() -> Result<()> {
    let document = render_error_json(&SnippetSourceError::new())?;
    assert_rejected_for_leaking(&document, "a marker-bearing cause must be rejected")
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

/// A plain source error that reports the excerpt-bearing error as its cause.
///
/// It implements no miette diagnostic, so `render_error_json` renders it
/// through the standard-error chain and copies each link's `Display` into
/// `causes` unchanged. That is the exposure the plain path really has.
#[derive(Debug)]
struct SnippetSourceError {
    /// The excerpt-bearing cause, reached through `Error::source`.
    cause: SnippetSource,
}

impl SnippetSourceError {
    /// Builds the fixture with its excerpt-bearing cause attached.
    fn new() -> Self {
        Self {
            cause: SnippetSource,
        }
    }
}

impl fmt::Display for SnippetSourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("outer plain error")
    }
}

impl StdError for SnippetSourceError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(&self.cause)
    }
}

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
/// The top-level rejection case proves the guard catches a leak at the shallowest
/// depth. It would still pass if the serializer never emitted a `related` entry
/// carrying a cause chain, leaving the recursion as dead code for a field shape
/// that never occurs. This case routes a leak through `render_diagnostic_json`
/// itself, so the serializer's own output is the evidence that the nested depth
/// exists and is reached. Breaking `visit_causes`' descent fails this case while
/// the shallow one still passes.
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
    assert_rejected_for_leaking(
        &document,
        "the serializer's own related leak must be rejected",
    )
}
