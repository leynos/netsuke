//! Semantic linting of Netsuke manifests.
//!
//! The linter analyses Netsuke's own compiler artefacts rather than the YAML
//! text: the authored source with its spans, the expanded and rendered
//! manifest, and the lowered build graph. That is what lets a rule tell an
//! order-only directory dependency from a content dependency, or recognize
//! that a literal path in a recipe is another target's output.
//!
//! `docs/netsuke-linter-design.md` specifies the rule model, the stage hooks,
//! the suppression grammar, and the output schemas;
//! `docs/adr-018-manifest-linting-under-netsuke-check.md` records the decisions
//! behind them.

pub mod document;
mod document_build;
pub mod engine;
pub mod finding;
pub mod policy;
pub mod registry;
pub mod report;
pub mod resolve;
pub mod rule;
pub mod rules;
mod scalar_span;
pub mod severity;
pub mod suppress;
#[cfg(test)]
pub mod test_support;

#[cfg(test)]
#[path = "example_manifest_tests.rs"]
mod example_manifest_tests;

use crate::ast::NetsukeManifest;
use crate::ir::BuildGraph;

pub use document_build::ParseFailure;
pub use engine::Outcome;
pub use finding::{Finding, Location};
pub use policy::{Policy, PolicyError};
pub use registry::{catalogue, meta_by_name};
pub use report::{Bounds, Report};
pub use rule::{Category, RuleMeta, Stage};
pub use severity::{DefaultSeverity, FAIL_ON_VALUES, FailOn, SEVERITY_VALUES, Severity};

/// The compiler artefacts one lint run inspects.
pub struct Request<'a> {
    /// The manifest source text, exactly as read from disk.
    pub source: String,
    /// The expanded and rendered manifest.
    pub manifest: &'a NetsukeManifest,
    /// The build graph lowered from that manifest.
    pub graph: &'a BuildGraph,
}

/// Lint one manifest under `policy`.
///
/// # Errors
///
/// Returns a [`ParseFailure`] when the source cannot be indexed. A manifest
/// that reached this point has already parsed for the compiler, so this
/// indicates a scanner disagreement rather than an authoring mistake.
pub fn analyse(request: Request<'_>, policy: &Policy) -> Result<Outcome, ParseFailure> {
    let document = document::Document::parse(request.source)?;
    let analysis = engine::Analysis {
        document: &document,
        manifest: request.manifest,
        graph: request.graph,
    };
    Ok(engine::run(&analysis, policy))
}
