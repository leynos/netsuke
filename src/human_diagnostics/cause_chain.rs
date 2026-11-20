//! Ordinary failure causes rendered when no richer diagnostic is available.
//!
//! A failure such as a graph rejection or a Ninja-synthesis error carries its
//! explanation as a plain [`std::error::Error`] beneath one or more `anyhow`
//! contexts. The human path selected only a `RunnerError`, a `ManifestError`,
//! or a `miette::Report`, so those failures printed their summary and dropped
//! the cause that named the fault.
//!
//! This module turns the remaining causes into a diagnostic, which lets
//! `miette` render them in the same layout as every other failure rather than
//! adding a second output shape. The chain is deduplicated against the outer
//! context because that line is already printed above it.

use std::error::Error as StdError;
use std::fmt;

use anyhow::Error;
use miette::Diagnostic;

/// Ordinary causes of a failure, linked for `miette`'s cause-chain renderer.
///
/// Each node prints one cause and exposes the next through
/// [`StdError::source`], so `miette` draws the same `├─▶`/`╰─▶` chain it draws
/// for a diagnostic that declares its own source.
///
/// The chain owns its messages rather than borrowing the failure chain. A
/// rendered report outlives the `anyhow::Error` it was built from at several
/// call sites, and copying each message keeps this type independent of that
/// lifetime without changing the text a reader sees.
#[derive(Debug)]
pub(super) struct CauseChain {
    /// The message this link prints.
    message: String,
    /// The next cause, when the chain has one.
    next: Option<Box<Self>>,
}

impl CauseChain {
    /// Build a chain from already-deduplicated cause messages.
    ///
    /// Returns `None` for an empty sequence, which is how a failure with no
    /// ordinary cause stays a single context line.
    pub(super) fn from_messages(messages: Vec<String>) -> Option<Self> {
        // Build from the tail so each node owns the link that follows it.
        let mut chain: Option<Box<Self>> = None;
        for message in messages.into_iter().rev() {
            chain = Some(Box::new(Self {
                message,
                next: chain,
            }));
        }
        chain.map(|head| *head)
    }
}

impl fmt::Display for CauseChain {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl StdError for CauseChain {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.next
            .as_ref()
            .map(|next| next.as_ref() as &(dyn StdError + 'static))
    }
}

impl Diagnostic for CauseChain {}

/// Collect the ordinary causes of a failure, dropping repeated messages.
///
/// `anyhow` prints only the outer context with `{}`; the rest of the chain is
/// what names the fault. A context may legitimately repeat the message beneath
/// it, or repeat the outer line the report already prints, so equal neighbours
/// are dropped rather than shown twice.
///
/// The outer message participates in the comparison for the same reason: a
/// chain that restates its summary beneath itself would otherwise print that
/// sentence twice, once above the rendering and once inside it.
pub(super) fn distinct_causes(err: &Error) -> Vec<String> {
    let outer = err.to_string();
    let mut previous = outer.clone();
    let mut causes = Vec::new();
    for cause in err.chain().skip(1) {
        let message = cause.to_string();
        if message != outer && message != previous {
            previous.clone_from(&message);
            causes.push(message);
        }
    }
    causes
}
