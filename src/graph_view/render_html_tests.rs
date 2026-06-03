//! Tests for the HTML renderer.

use camino::Utf8PathBuf;

use crate::graph_view::render::GraphRenderer;
use crate::graph_view::{EdgeClass, EdgeView, GraphView, NodeKind, NodeView};

use super::HtmlRenderer;

fn small_view() -> GraphView {
    GraphView {
        default_targets: vec![Utf8PathBuf::from("out/app")],
        nodes: vec![
            NodeView {
                path: Utf8PathBuf::from("out/app"),
                kind: NodeKind::Target {
                    phony: false,
                    always: false,
                },
                action_id: Some("h".into()),
                description: Some("link app".into()),
            },
            NodeView {
                path: Utf8PathBuf::from("src/main.c"),
                kind: NodeKind::Source,
                action_id: None,
                description: None,
            },
        ],
        edges: vec![EdgeView {
            from: Utf8PathBuf::from("src/main.c"),
            to: Utf8PathBuf::from("out/app"),
            class: EdgeClass::Explicit,
        }],
        limit: None,
    }
}

fn render(locale: Option<&str>, view: &GraphView) -> String {
    let mut out = Vec::new();
    HtmlRenderer::new(locale)
        .render(view, &mut out)
        .expect("render html");
    String::from_utf8(out).expect("utf-8")
}

#[test]
fn document_contains_required_structure() {
    let html = render(None, &small_view());
    for fragment in [
        "<!doctype html>\n",
        "<html lang=\"en\"",
        "<title>",
        "<svg",
        "aria-labelledby",
        "<details",
        "<noscript>",
        "digraph netsuke",
    ] {
        assert!(html.contains(fragment), "missing fragment: {fragment}");
    }
}

#[test]
fn document_has_no_external_references() {
    let html = render(None, &small_view());
    for attr in ["href=\"http", "src=\"http", "url(http"] {
        assert!(!html.contains(attr), "external resource reference: {attr}");
    }
}

#[test]
fn nodes_and_edges_carry_accessibility_metadata() {
    let html = render(None, &small_view());
    assert!(html.contains("aria-label=\"out/app\""));
    assert!(html.contains("aria-label=\"src/main.c\""));
    // The edge title text encodes the relationship for hover/screen reader.
    assert!(html.contains("src/main.c → out/app"));
}

#[test]
fn rendering_is_byte_identical_across_runs() {
    let view = small_view();
    let first = render(None, &view);
    let second = render(None, &view);
    assert_eq!(first, second);
}

#[test]
fn locale_attribute_reflects_locale_argument() {
    let html = render(Some("es-ES"), &small_view());
    assert!(html.contains("<html lang=\"es-ES\""));
}

#[test]
fn outline_lists_targets_and_inputs() {
    let html = render(None, &small_view());
    // Target appears in the outline; source files are not listed as separate
    // outline entries because they have no inputs.
    assert!(html.contains("<details"));
    assert!(html.contains("<code>out/app</code>"));
    assert!(html.contains("<code>src/main.c</code>"));
}

#[test]
fn implicit_dep_edge_emits_implicit_dep_class() {
    let mut view = small_view();
    view.nodes.push(NodeView {
        path: Utf8PathBuf::from("include/config.h"),
        kind: NodeKind::Source,
        action_id: None,
        description: None,
    });
    view.edges.push(EdgeView {
        from: Utf8PathBuf::from("include/config.h"),
        to: Utf8PathBuf::from("out/app"),
        class: EdgeClass::ImplicitDep,
    });
    let html = render(None, &view);
    assert!(
        html.contains(".edge.implicit-dep"),
        "stylesheet should define .edge.implicit-dep"
    );
    assert!(
        html.contains("class=\"edge implicit-dep\""),
        "implicit-dep edge should carry the implicit-dep class"
    );
}

#[test]
fn paths_with_angle_brackets_are_escaped() {
    let mut view = small_view();
    view.nodes.push(NodeView {
        path: Utf8PathBuf::from("a<b>"),
        kind: NodeKind::Source,
        action_id: None,
        description: None,
    });
    let html = render(None, &view);
    assert!(html.contains("a&lt;b&gt;"));
}
