//! Self-contained HTML renderer for [`GraphView`].
//!
//! Produces a single offline-safe HTML document containing:
//!
//! 1. A server-rendered SVG laid out as a left-to-right hierarchical DAG.
//!    The SVG carries `role="img"`, `aria-labelledby`, per-node `aria-label`s,
//!    and per-edge `<title>` elements so screen readers can navigate the
//!    structure.
//! 2. A `<details>` block listing every target and its inputs as a plain-text
//!    outline — the primary screen-reader path while structured `--json`
//!    inspection (roadmap 3.15.6) remains a follow-up.
//! 3. A `<noscript>` block restating the dependency graph and the DOT source
//!    verbatim so the page is fully functional with JavaScript disabled.
//!
//! No JavaScript is required at view time. No external assets are referenced.

mod escape;
mod layout;
mod noscript;
mod outline;
mod style;
mod svg;

use std::io::Write;

use crate::localization::{self, keys};

use super::GraphView;
use super::render::{GraphRenderError, GraphRenderer};

use escape::{escape_attr, escape_text};
use layout::layout_positions;
use noscript::write_noscript;
use outline::write_outline;
use style::write_inline_style;
use svg::{SvgHeader, write_svg};

/// Render adapter producing a self-contained HTML page.
#[derive(Debug, Clone)]
pub struct HtmlRenderer {
    locale: String,
}

impl Default for HtmlRenderer {
    fn default() -> Self {
        Self::new(None)
    }
}

impl HtmlRenderer {
    /// Construct a new HTML renderer.
    ///
    /// The optional `locale` value sets the resulting page's `<html lang>`
    /// attribute. When `None` the renderer defaults to `en` to keep the page
    /// valid for assistive tooling.
    #[must_use]
    pub fn new(locale: Option<&str>) -> Self {
        Self {
            locale: locale.unwrap_or("en").to_owned(),
        }
    }
}

impl GraphRenderer for HtmlRenderer {
    fn render(&self, view: &GraphView, sink: &mut dyn Write) -> Result<(), GraphRenderError> {
        let positions = layout_positions(view);
        let title = localized(keys::GRAPH_HTML_TITLE);
        let heading = localized(keys::GRAPH_HTML_HEADING);
        let description = format!(
            "{} ({}, {})",
            localized(keys::GRAPH_HTML_DESCRIPTION),
            view.nodes.len(),
            view.edges.len()
        );
        let outline_summary = localized(keys::GRAPH_HTML_OUTLINE_SUMMARY);
        let noscript_notice = localized(keys::GRAPH_HTML_NOSCRIPT_NOTICE);

        writeln!(sink, "<!doctype html>")?;
        writeln!(sink, "<html lang=\"{}\">", escape_attr(&self.locale))?;
        writeln!(sink, "<head>")?;
        writeln!(sink, "  <meta charset=\"utf-8\">")?;
        writeln!(
            sink,
            "  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">"
        )?;
        writeln!(sink, "  <title>{}</title>", escape_text(&title))?;
        write_inline_style(sink)?;
        writeln!(sink, "</head>")?;
        writeln!(sink, "<body>")?;
        writeln!(sink, "  <h1>{}</h1>", escape_text(&heading))?;
        write_svg(
            sink,
            view,
            &positions,
            SvgHeader {
                title: &title,
                description: &description,
            },
        )?;
        write_outline(sink, view, &outline_summary)?;
        write_noscript(sink, view, &noscript_notice)?;
        writeln!(sink, "</body>")?;
        writeln!(sink, "</html>")?;
        Ok(())
    }
}

pub(super) fn localized(key: &'static str) -> String {
    localization::message(key).to_string()
}

#[cfg(test)]
mod tests;
