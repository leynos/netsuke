//! Type-check Netsuke's public lint surface.
//!
//! The generic helpers exercise all four stage-rule signatures, and the
//! uninhabited placeholders implement each rule trait without inventing a
//! production rule.

use netsuke::ast::NetsukeManifest;
use netsuke::ir::BuildGraph;
use netsuke::lint::document::Document;
use netsuke::lint::rule::{
    DirectiveContext, DirectiveRule, DocumentRule, FindingSink, GraphContext, GraphRule,
    ManifestContext, ManifestRule,
};
use netsuke::lint::{Outcome, ParseFailure, Policy, Request, RuleMeta, analyse};

/// Type-check the document-stage public rule signature.
pub fn check_document_rule<R: DocumentRule>(
    rule: &R,
    document: &Document,
    sink: &mut FindingSink<'_>,
) {
    rule.check(document, sink);
}

/// Type-check the manifest-stage public rule signature.
pub fn check_manifest_rule<R: ManifestRule>(
    rule: &R,
    context: &ManifestContext<'_>,
    sink: &mut FindingSink<'_>,
) {
    rule.check(context, sink);
}

/// Type-check the graph-stage public rule signature.
pub fn check_graph_rule<R: GraphRule>(
    rule: &R,
    context: &GraphContext<'_>,
    sink: &mut FindingSink<'_>,
) {
    rule.check(context, sink);
}

/// Type-check the directive-stage public rule signature.
pub fn check_directive_rule<R: DirectiveRule>(
    rule: &R,
    context: &DirectiveContext<'_>,
    sink: &mut FindingSink<'_>,
) {
    rule.check(context, sink);
}

/// Build an externally constructible lint request.
#[must_use]
pub const fn request<'a>(manifest: &'a NetsukeManifest, graph: &'a BuildGraph) -> Request<'a> {
    Request {
        source: String::new(),
        manifest,
        graph,
    }
}

/// Type-check the analysis function at the external API boundary.
///
/// # Errors
///
/// Returns the parse failure `analyse` reports for the request's source.
pub fn analyse_request(request: Request<'_>, policy: &Policy) -> Result<Outcome, ParseFailure> {
    analyse(request, policy)
}

/// Parse an empty document through the public constructor.
///
/// # Errors
///
/// Returns the parse failure `Document::parse` reports.
pub fn parse_empty_document() -> Result<Document, ParseFailure> {
    Document::parse(String::new())
}

/// Construct the default lint policy.
#[must_use]
pub fn default_policy() -> Policy {
    Policy::defaults()
}

/// An uninhabited placeholder for document-rule signature checking.
pub enum NeverDocumentRule {}

impl DocumentRule for NeverDocumentRule {
    fn meta(&self) -> &'static RuleMeta {
        match *self {}
    }

    fn check(&self, _: &Document, _: &mut FindingSink<'_>) {
        match *self {}
    }
}

/// An uninhabited placeholder for manifest-rule signature checking.
pub enum NeverManifestRule {}

impl ManifestRule for NeverManifestRule {
    fn meta(&self) -> &'static RuleMeta {
        match *self {}
    }

    fn check(&self, _: &ManifestContext<'_>, _: &mut FindingSink<'_>) {
        match *self {}
    }
}

/// An uninhabited placeholder for graph-rule signature checking.
pub enum NeverGraphRule {}

impl GraphRule for NeverGraphRule {
    fn meta(&self) -> &'static RuleMeta {
        match *self {}
    }

    fn check(&self, _: &GraphContext<'_>, _: &mut FindingSink<'_>) {
        match *self {}
    }
}

/// An uninhabited placeholder for directive-rule signature checking.
pub enum NeverDirectiveRule {}

impl DirectiveRule for NeverDirectiveRule {
    fn meta(&self) -> &'static RuleMeta {
        match *self {}
    }

    fn check(&self, _: &DirectiveContext<'_>, _: &mut FindingSink<'_>) {
        match *self {}
    }
}
