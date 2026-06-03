//! Deterministic Graphviz DOT renderer for [`GraphView`].

use std::io::Write;

use camino::Utf8Path;

use super::render::{GraphRenderError, GraphRenderer};
use super::{EdgeClass, GraphView, NodeKind};

/// Render adapter producing Graphviz DOT output.
#[derive(Debug, Default, Clone, Copy)]
pub struct DotRenderer;

impl DotRenderer {
    /// Construct a new DOT renderer.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}

impl GraphRenderer for DotRenderer {
    fn render(&self, view: &GraphView, sink: &mut dyn Write) -> Result<(), GraphRenderError> {
        writeln!(sink, "digraph netsuke {{")?;
        writeln!(sink, "  rankdir=\"LR\"")?;
        writeln!(sink, "  node [shape=box, fontsize=10]")?;
        writeln!(sink, "  edge [fontsize=10]")?;

        for node in &view.nodes {
            write_node(
                sink,
                node.path.as_path(),
                node.kind,
                node.description.as_deref(),
            )?;
        }

        for edge in &view.edges {
            write_edge(sink, edge.from.as_path(), edge.to.as_path(), edge.class)?;
        }

        if !view.default_targets.is_empty() {
            writeln!(sink, "  // default targets:")?;
            for target in &view.default_targets {
                writeln!(sink, "  // - {}", escape_dot(target.as_str()))?;
            }
        }

        writeln!(sink, "}}")?;
        Ok(())
    }
}

fn write_node(
    sink: &mut dyn Write,
    path: &Utf8Path,
    kind: NodeKind,
    description: Option<&str>,
) -> Result<(), GraphRenderError> {
    let path_str = escape_dot(path.as_str());
    let label = description.filter(|s| !s.is_empty()).map_or_else(
        || path_str.clone(),
        |d| format!("{}\\n{}", path_str, escape_dot(d)),
    );
    let attrs = match kind {
        NodeKind::Source => format!("label=\"{label}\""),
        NodeKind::Target { phony: true, .. } => {
            format!("label=\"{label}\", style=dashed, color=\"#666666\"")
        }
        NodeKind::Target {
            phony: false,
            always: true,
        } => format!("label=\"{label}\", color=\"#cc6600\""),
        NodeKind::Target {
            phony: false,
            always: false,
        } => format!("label=\"{label}\", color=\"#003366\""),
    };
    writeln!(sink, "  \"{path_str}\" [{attrs}]")?;
    Ok(())
}

fn write_edge(
    sink: &mut dyn Write,
    from: &Utf8Path,
    to: &Utf8Path,
    class: EdgeClass,
) -> Result<(), GraphRenderError> {
    let from_str = escape_dot(from.as_str());
    let to_str = escape_dot(to.as_str());
    let attrs = match class {
        EdgeClass::Explicit => "",
        EdgeClass::ImplicitDep => " [style=bold]",
        EdgeClass::ImplicitOutput => " [style=dotted]",
        EdgeClass::OrderOnly => " [style=dashed]",
    };
    writeln!(sink, "  \"{from_str}\" -> \"{to_str}\"{attrs}")?;
    Ok(())
}

/// Escape a string for inclusion inside a double-quoted Graphviz identifier or
/// label.
fn escape_dot(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;

    use super::*;
    use crate::graph_view::{EdgeView, GraphView, NodeView};

    fn empty_view() -> GraphView {
        GraphView {
            default_targets: Vec::new(),
            nodes: Vec::new(),
            edges: Vec::new(),
            limit: None,
        }
    }

    fn render(view: &GraphView) -> String {
        let mut out = Vec::new();
        DotRenderer::new()
            .render(view, &mut out)
            .expect("render dot");
        String::from_utf8(out).expect("utf-8")
    }

    #[test]
    fn empty_graph_produces_well_formed_digraph() {
        let dot = render(&empty_view());
        assert!(dot.starts_with("digraph netsuke {"));
        assert!(dot.trim_end().ends_with('}'));
    }

    #[test]
    fn renders_source_target_and_explicit_edge() {
        let view = GraphView {
            default_targets: vec![Utf8PathBuf::from("out/a.o")],
            nodes: vec![
                NodeView {
                    path: Utf8PathBuf::from("out/a.o"),
                    kind: NodeKind::Target {
                        phony: false,
                        always: false,
                    },
                    action_id: Some("h".into()),
                    description: Some("compile a".into()),
                },
                NodeView {
                    path: Utf8PathBuf::from("src/a.c"),
                    kind: NodeKind::Source,
                    action_id: None,
                    description: None,
                },
            ],
            edges: vec![EdgeView {
                from: Utf8PathBuf::from("src/a.c"),
                to: Utf8PathBuf::from("out/a.o"),
                class: EdgeClass::Explicit,
            }],
            limit: None,
        };
        let dot = render(&view);
        assert!(dot.contains("\"src/a.c\" [label=\"src/a.c\"]"));
        assert!(dot.contains("\"out/a.o\" [label=\"out/a.o\\ncompile a\""));
        assert!(dot.contains("\"src/a.c\" -> \"out/a.o\"\n"));
        assert!(dot.contains("// default targets:"));
        assert!(dot.contains("// - out/a.o"));
    }

    #[test]
    fn order_only_and_implicit_edges_carry_style() {
        let view = GraphView {
            default_targets: Vec::new(),
            nodes: vec![NodeView {
                path: Utf8PathBuf::from("o"),
                kind: NodeKind::Source,
                action_id: None,
                description: None,
            }],
            edges: vec![
                EdgeView {
                    from: Utf8PathBuf::from("a"),
                    to: Utf8PathBuf::from("o"),
                    class: EdgeClass::OrderOnly,
                },
                EdgeView {
                    from: Utf8PathBuf::from("b"),
                    to: Utf8PathBuf::from("o"),
                    class: EdgeClass::ImplicitOutput,
                },
            ],
            limit: None,
        };
        let dot = render(&view);
        assert!(dot.contains("\"a\" -> \"o\" [style=dashed]"));
        assert!(dot.contains("\"b\" -> \"o\" [style=dotted]"));
    }

    #[test]
    fn implicit_dep_edges_are_bold() {
        let view = GraphView {
            default_targets: Vec::new(),
            nodes: vec![NodeView {
                path: Utf8PathBuf::from("o"),
                kind: NodeKind::Source,
                action_id: None,
                description: None,
            }],
            edges: vec![EdgeView {
                from: Utf8PathBuf::from("dep"),
                to: Utf8PathBuf::from("o"),
                class: EdgeClass::ImplicitDep,
            }],
            limit: None,
        };
        let dot = render(&view);
        assert!(dot.contains("\"dep\" -> \"o\" [style=bold]"));
    }

    #[test]
    fn paths_with_quotes_are_escaped() {
        let view = GraphView {
            default_targets: Vec::new(),
            nodes: vec![NodeView {
                path: Utf8PathBuf::from("a\"b"),
                kind: NodeKind::Source,
                action_id: None,
                description: None,
            }],
            edges: Vec::new(),
            limit: None,
        };
        let dot = render(&view);
        assert!(dot.contains("\"a\\\"b\""));
    }
}
