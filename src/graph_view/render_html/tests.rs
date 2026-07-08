//! Tests for the HTML renderer.

use anyhow::{Context, Result, ensure};
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

fn render(locale: Option<&str>, view: &GraphView) -> Result<String> {
    let mut out = Vec::new();
    HtmlRenderer::new(locale)
        .render(view, &mut out)
        .context("render html")?;
    String::from_utf8(out).context("utf-8")
}

#[test]
fn document_contains_required_structure() -> Result<()> {
    let html = render(None, &small_view())?;
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
        ensure!(html.contains(fragment), "missing fragment: {fragment}");
    }
    Ok(())
}

#[test]
fn document_has_no_external_references() -> Result<()> {
    let html = render(None, &small_view())?;
    for attr in ["href=\"http", "src=\"http", "url(http"] {
        ensure!(!html.contains(attr), "external resource reference: {attr}");
    }
    Ok(())
}

#[test]
fn nodes_and_edges_carry_accessibility_metadata() -> Result<()> {
    let html = render(None, &small_view())?;
    ensure!(
        html.contains("aria-label=\"out/app\""),
        "missing aria-label for out/app"
    );
    ensure!(
        html.contains("aria-label=\"src/main.c\""),
        "missing aria-label for src/main.c"
    );
    // The edge title text encodes the relationship for hover/screen reader.
    ensure!(
        html.contains("src/main.c → out/app"),
        "missing edge title text"
    );
    Ok(())
}

#[test]
fn rendering_is_byte_identical_across_runs() -> Result<()> {
    let view = small_view();
    let first = render(None, &view)?;
    let second = render(None, &view)?;
    ensure!(
        first == second,
        "rendered HTML should be byte-identical across runs"
    );
    Ok(())
}

#[test]
fn locale_attribute_reflects_locale_argument() -> Result<()> {
    let html = render(Some("es-ES"), &small_view())?;
    ensure!(
        html.contains("<html lang=\"es-ES\""),
        "lang attribute should reflect the requested locale"
    );
    Ok(())
}

#[test]
fn outline_lists_targets_and_inputs() -> Result<()> {
    let html = render(None, &small_view())?;
    // Target appears in the outline; source files are not listed as separate
    // outline entries because they have no inputs.
    ensure!(html.contains("<details"), "missing outline details element");
    ensure!(
        html.contains("<code>out/app</code>"),
        "missing outline entry for out/app"
    );
    ensure!(
        html.contains("<code>src/main.c</code>"),
        "missing outline entry for src/main.c"
    );
    Ok(())
}

fn assert_edge_class_rendered(
    extra_node: NodeView,
    extra_edge: EdgeView,
    edge_class_name: &str,
) -> Result<()> {
    let mut view = small_view();
    view.nodes.push(extra_node);
    view.edges.push(extra_edge);
    let html = render(None, &view)?;
    let selector = format!(".edge.{edge_class_name}");
    let class_attr = format!("class=\"edge {edge_class_name}\"");
    ensure!(
        html.contains(&selector),
        "stylesheet should define {selector}",
    );
    ensure!(
        html.contains(&class_attr),
        "{edge_class_name} edge should carry the {edge_class_name} class",
    );
    Ok(())
}

#[test]
fn implicit_dep_edge_emits_implicit_dep_class() -> Result<()> {
    assert_edge_class_rendered(
        NodeView {
            path: Utf8PathBuf::from("include/config.h"),
            kind: NodeKind::Source,
            action_id: None,
            description: None,
        },
        EdgeView {
            from: Utf8PathBuf::from("include/config.h"),
            to: Utf8PathBuf::from("out/app"),
            class: EdgeClass::ImplicitDep,
        },
        "implicit-dep",
    )
}

#[test]
fn implicit_output_edge_emits_implicit_output_class() -> Result<()> {
    assert_edge_class_rendered(
        NodeView {
            path: Utf8PathBuf::from("out/app.d"),
            kind: NodeKind::Target {
                phony: false,
                always: false,
            },
            action_id: Some("h".into()),
            description: None,
        },
        EdgeView {
            from: Utf8PathBuf::from("src/main.c"),
            to: Utf8PathBuf::from("out/app.d"),
            class: EdgeClass::ImplicitOutput,
        },
        "implicit-output",
    )
}

#[test]
fn order_only_edge_emits_order_only_class() -> Result<()> {
    assert_edge_class_rendered(
        NodeView {
            path: Utf8PathBuf::from("build-dir"),
            kind: NodeKind::Source,
            action_id: None,
            description: None,
        },
        EdgeView {
            from: Utf8PathBuf::from("build-dir"),
            to: Utf8PathBuf::from("out/app"),
            class: EdgeClass::OrderOnly,
        },
        "order-only",
    )
}

#[test]
fn paths_with_angle_brackets_are_escaped() -> Result<()> {
    let mut view = small_view();
    view.nodes.push(NodeView {
        path: Utf8PathBuf::from("a<b>"),
        kind: NodeKind::Source,
        action_id: None,
        description: None,
    });
    let html = render(None, &view)?;
    ensure!(
        html.contains("a&lt;b&gt;"),
        "angle brackets should be HTML-escaped"
    );
    Ok(())
}
