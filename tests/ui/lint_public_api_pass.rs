//! Compile-pass fixture for Netsuke's public lint surface.
//!
//! Compiled by `tests/command_env_ui_tests.rs` as an external crate. The
//! generic helpers below exercise all four stage-rule signatures without
//! inventing an implementation of a production rule.

use netsuke::ast::NetsukeManifest;
use netsuke::ir::BuildGraph;
use netsuke::lint::document::Document;
use netsuke::lint::rule::{
    DirectiveContext, DirectiveRule, DocumentRule, FindingSink, GraphContext, GraphRule,
    ManifestContext, ManifestRule,
};
use netsuke::lint::{Policy, Request, analyse};

/// Type-check the document-stage public rule signature.
fn check_document_rule<R: DocumentRule>(rule: &R, document: &Document, sink: &mut FindingSink<'_>) {
    rule.check(document, sink);
}

/// Type-check the manifest-stage public rule signature.
fn check_manifest_rule<R: ManifestRule>(
    rule: &R,
    context: &ManifestContext<'_>,
    sink: &mut FindingSink<'_>,
) {
    rule.check(context, sink);
}

/// Type-check the graph-stage public rule signature.
fn check_graph_rule<R: GraphRule>(
    rule: &R,
    context: &GraphContext<'_>,
    sink: &mut FindingSink<'_>,
) {
    rule.check(context, sink);
}

/// Type-check the directive-stage public rule signature.
fn check_directive_rule<R: DirectiveRule>(
    rule: &R,
    context: &DirectiveContext<'_>,
    sink: &mut FindingSink<'_>,
) {
    rule.check(context, sink);
}

/// Build an externally constructible lint request.
fn request<'a>(manifest: &'a NetsukeManifest, graph: &'a BuildGraph) -> Request<'a> {
    Request {
        source: String::new(),
        manifest,
        graph,
    }
}

/// Type-check the analysis function at the external API boundary.
fn analyse_request(request: Request<'_>, policy: &Policy) {
    let _ = analyse(request, policy);
}

/// Exercise the non-rule public lint types without executing an analysis.
fn main() {
    let _ = Document::parse(String::new());
    let _ = Policy::defaults();
    let _ = check_document_rule::<NeverDocumentRule>;
    let _ = check_manifest_rule::<NeverManifestRule>;
    let _ = check_graph_rule::<NeverGraphRule>;
    let _ = check_directive_rule::<NeverDirectiveRule>;
    let _ = request;
    let _ = analyse_request;
}

/// An uninhabited placeholder for document-rule signature checking.
enum NeverDocumentRule {}

impl DocumentRule for NeverDocumentRule {
    fn meta(&self) -> &'static netsuke::lint::RuleMeta {
        match *self {}
    }

    fn check(&self, _: &Document, _: &mut FindingSink<'_>) {
        match *self {}
    }
}

/// An uninhabited placeholder for manifest-rule signature checking.
enum NeverManifestRule {}

impl ManifestRule for NeverManifestRule {
    fn meta(&self) -> &'static netsuke::lint::RuleMeta {
        match *self {}
    }

    fn check(&self, _: &ManifestContext<'_>, _: &mut FindingSink<'_>) {
        match *self {}
    }
}

/// An uninhabited placeholder for graph-rule signature checking.
enum NeverGraphRule {}

impl GraphRule for NeverGraphRule {
    fn meta(&self) -> &'static netsuke::lint::RuleMeta {
        match *self {}
    }

    fn check(&self, _: &GraphContext<'_>, _: &mut FindingSink<'_>) {
        match *self {}
    }
}

/// An uninhabited placeholder for directive-rule signature checking.
enum NeverDirectiveRule {}

impl DirectiveRule for NeverDirectiveRule {
    fn meta(&self) -> &'static netsuke::lint::RuleMeta {
        match *self {}
    }

    fn check(&self, _: &DirectiveContext<'_>, _: &mut FindingSink<'_>) {
        match *self {}
    }
}
