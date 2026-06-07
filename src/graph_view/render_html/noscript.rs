//! `<noscript>` fallback block that restates the dependency graph verbatim as
//! Graphviz DOT, keeping the page fully functional with JavaScript disabled.

use std::io::{self, Write};

use crate::graph_view::GraphView;
use crate::graph_view::render::{GraphRenderError, GraphRenderer};
use crate::graph_view::render_dot::DotRenderer;

use super::escape::escape_text;

pub(super) fn write_noscript(
    sink: &mut dyn Write,
    view: &GraphView,
    notice: &str,
) -> Result<(), GraphRenderError> {
    writeln!(sink, "  <noscript>")?;
    writeln!(sink, "    <p>{}</p>", escape_text(notice))?;
    writeln!(sink, "    <pre><code>")?;
    let mut dot_buf: Vec<u8> = Vec::new();
    DotRenderer::new().render(view, &mut dot_buf)?;
    let dot = String::from_utf8(dot_buf).map_err(|err| {
        GraphRenderError::from(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("DOT renderer produced non-UTF-8 output: {err}"),
        ))
    })?;
    write!(sink, "{}", escape_text(&dot))?;
    writeln!(sink, "</code></pre>")?;
    writeln!(sink, "  </noscript>")?;
    Ok(())
}
