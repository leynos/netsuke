//! Renderer port for [`super::GraphView`].
//!
//! Each renderer adapter (DOT, HTML, future JSON) implements
//! [`GraphRenderer`] and consumes a [`GraphView`] plus a polymorphic
//! [`std::io::Write`] sink.

use std::io;

use thiserror::Error;

use super::GraphView;

/// Trait implemented by every renderer adapter consuming a [`GraphView`].
pub trait GraphRenderer {
    /// Render the graph view into the provided sink.
    ///
    /// # Errors
    ///
    /// Returns [`GraphRenderError`] when writing to the sink fails or the
    /// renderer cannot format part of the view.
    fn render(&self, view: &GraphView, sink: &mut dyn io::Write) -> Result<(), GraphRenderError>;
}

/// Errors produced by a [`GraphRenderer`] implementation.
#[derive(Debug, Error)]
pub enum GraphRenderError {
    /// I/O failure encountered while writing the rendered graph.
    #[error("I/O failure while rendering graph")]
    Io {
        /// Underlying I/O error.
        #[source]
        source: io::Error,
    },
    /// Formatting failure raised by the renderer's internal writer.
    #[error("formatting failure while rendering graph")]
    Format {
        /// Underlying formatter error.
        #[source]
        source: std::fmt::Error,
    },
}

impl From<io::Error> for GraphRenderError {
    fn from(source: io::Error) -> Self {
        Self::Io { source }
    }
}

impl From<std::fmt::Error> for GraphRenderError {
    fn from(source: std::fmt::Error) -> Self {
        Self::Format { source }
    }
}
