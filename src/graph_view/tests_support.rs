//! Shared fixtures and rendering helpers for the `GraphView` test modules:
//! action and edge constructors plus fallible DOT and HTML render wrappers.

use anyhow::{Context, Result};
use camino::Utf8PathBuf;

use crate::ast::Recipe;
use crate::graph_view::GraphView;
use crate::graph_view::render::GraphRenderer;
use crate::graph_view::render_dot::DotRenderer;
use crate::graph_view::render_html::HtmlRenderer;
use crate::ir::{Action, BuildEdge, BuildGraph};

pub(super) fn make_action(description: Option<&str>) -> Action {
    Action {
        recipe: Recipe::Command {
            command: "echo".into(),
        },
        description: description.map(str::to_owned),
        depfile: None,
        deps_format: None,
        pool: None,
        restat: false,
    }
}

pub(super) fn p(s: &str) -> Utf8PathBuf {
    Utf8PathBuf::from(s)
}

pub(super) fn render_dot(view: &GraphView) -> Result<String> {
    let mut out = Vec::new();
    DotRenderer::new()
        .render(view, &mut out)
        .context("render DOT graph")?;
    String::from_utf8(out).context("DOT renderer emits UTF-8")
}

pub(super) fn render_html(view: &GraphView) -> Result<String> {
    let mut out = Vec::new();
    HtmlRenderer::new(Some("en-US"))
        .render(view, &mut out)
        .context("render HTML graph")?;
    String::from_utf8(out).context("HTML renderer emits UTF-8")
}

#[derive(Default, Clone, Copy)]
pub(super) struct EdgeFixture<'a> {
    pub(super) action_id: &'a str,
    pub(super) inputs: &'a [&'a str],
    pub(super) implicit_deps: &'a [&'a str],
    pub(super) explicit_outputs: &'a [&'a str],
    pub(super) implicit_outputs: &'a [&'a str],
    pub(super) order_only_deps: &'a [&'a str],
    pub(super) phony: bool,
    pub(super) always: bool,
}

pub(super) fn add_edge(graph: &mut BuildGraph, fixture: EdgeFixture<'_>) {
    let edge = BuildEdge {
        action_id: fixture.action_id.into(),
        inputs: fixture.inputs.iter().map(|s| p(s)).collect(),
        implicit_deps: fixture.implicit_deps.iter().map(|s| p(s)).collect(),
        explicit_outputs: fixture.explicit_outputs.iter().map(|s| p(s)).collect(),
        implicit_outputs: fixture.implicit_outputs.iter().map(|s| p(s)).collect(),
        order_only_deps: fixture.order_only_deps.iter().map(|s| p(s)).collect(),
        phony: fixture.phony,
        always: fixture.always,
    };
    for out in &edge.explicit_outputs {
        graph.targets.insert(out.clone(), edge.clone());
    }
    for out in &edge.implicit_outputs {
        graph.targets.insert(out.clone(), edge.clone());
    }
}
