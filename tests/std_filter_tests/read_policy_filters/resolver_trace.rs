//! Correlate an `strace` log with the fixtures the manual-resolver child read.
//!
//! `resolver_fallback_tests` runs the child under an injected `openat2`
//! failure; this module owns what the resulting trace must show. Each fixture
//! the child reads has a basename and an expected resolver outcome, and every
//! assertion is made against only the trace lines that mention that basename,
//! so a line about another fixture cannot satisfy it.
//!
//! The checks are deliberately syscall-shaped. A refused link must show the
//! injected `openat2` failure, then an `openat` refused with `ELOOP`, and no
//! `readlinkat` at all: dereferencing the final component is the pre-fix
//! behaviour, and only that syscall records it. The opt-in must show the
//! `readlinkat`, and a regular-file control must show an `openat` that
//! returned a descriptor.
//!
//! The separation keeps the spawn, the availability probe and the deadline in
//! `resolver_fallback_tests`, which knows nothing about how a trace is read.
use anyhow::{Result, ensure};

use super::resolver_fallback_tests::CASES;
use super::{FilterCase, Policy};

/// What the manual resolver is expected to do with one traced basename.
#[derive(Clone, Copy, Debug)]
enum TracePath {
    /// The final component is a link the no-follow policy refuses.
    RefusedLink,
    /// The final component is a link the opt-in resolves.
    FollowedLink,
    /// The final component is an ordinary file.
    RegularFile,
}

/// One fixture the child reads and the trace must account for.
///
/// Visible to the parent module because [`assert_trace`] takes one, while the
/// fields stay private: only this module builds or reads an expectation.
#[derive(Clone, Debug)]
pub(super) struct TraceExpectation {
    /// Fixture basename the trace must mention.
    name: String,
    /// What the manual resolver must do with that basename.
    path: TracePath,
}

impl TraceExpectation {
    /// The expectations for `case` and `policy`.
    fn for_symlink(case: FilterCase, policy: Policy) -> Self {
        Self {
            name: link_fixture_name(case, policy),
            path: if policy.follows() {
                TracePath::FollowedLink
            } else {
                TracePath::RefusedLink
            },
        }
    }

    /// The expectation for the regular-file control of `case`.
    fn for_control(case: FilterCase) -> Self {
        Self {
            name: control_fixture_name(case),
            path: TracePath::RegularFile,
        }
    }
}

/// The basename of the symlink fixture for `case` under `policy`.
pub(super) fn link_fixture_name(case: FilterCase, policy: Policy) -> String {
    format!("link_{}_{}", case.name, policy.suffix())
}

/// The basename of the regular-file control for `case`.
pub(super) fn control_fixture_name(case: FilterCase) -> String {
    format!("control_{}", case.name)
}

/// Every fixture the traced child reads, in trace-correlation order.
pub(super) fn trace_expectations() -> Vec<TraceExpectation> {
    let mut expectations = Vec::with_capacity(CASES.len() * (Policy::ALL.len() + 1));
    for case in CASES {
        for policy in Policy::ALL {
            expectations.push(TraceExpectation::for_symlink(case, policy));
        }
        expectations.push(TraceExpectation::for_control(case));
    }
    expectations
}

/// One syscall shape the trace correlation looks for.
///
/// The shapes are matched by the substrings a line must contain and, for a
/// successful open, must not contain. Stating them as data rather than as a
/// chain of conditions keeps the matcher small and lets each shape be read on
/// its own.
#[derive(Clone, Copy, Debug)]
enum TraceLine {
    /// `openat2` refused by the injected fault.
    RefusedOpenat2,
    /// Any `openat` attempt, refused or not.
    Openat,
    /// `openat` refused with `ELOOP`.
    RefusedOpenat,
    /// `openat` that returned a descriptor.
    SuccessfulOpenat,
    /// A `readlinkat` dereference.
    Readlinkat,
}

impl TraceLine {
    /// Substrings a matching trace line must all contain.
    fn required(self) -> &'static [&'static str] {
        match self {
            Self::RefusedOpenat2 => &["openat2(", "= -1 EPERM"],
            Self::Openat | Self::RefusedOpenat | Self::SuccessfulOpenat => &["openat("],
            Self::Readlinkat => &["readlinkat("],
        }
    }

    /// Substrings that disqualify a trace line.
    ///
    /// An `openat` is successful by the absence of an error marker, so the
    /// benign shape has to exclude the one prefix every failure carries.
    fn forbidden(self) -> &'static [&'static str] {
        match self {
            Self::SuccessfulOpenat => &["= -1 "],
            Self::RefusedOpenat2 | Self::Openat | Self::RefusedOpenat | Self::Readlinkat => &[],
        }
    }
}

/// Whether `line` shows the syscall `shape`.
fn line_matches(shape: TraceLine, line: &str) -> bool {
    shape.required().iter().all(|needle| line.contains(needle))
        && !shape.forbidden().iter().any(|needle| line.contains(needle))
}

/// The trace lines that mention one fixture, and the outcome they must show.
struct TraceWindow<'a> {
    /// Fixture basename, for failure diagnostics.
    name: &'a str,
    /// Lines mentioning that basename; a trace about another fixture cannot
    /// satisfy any assertion made through this window.
    lines: Vec<&'a str>,
    /// What the manual resolver must do with the fixture.
    path: TracePath,
}

impl<'a> TraceWindow<'a> {
    /// Collect the lines of `trace` that mention the expected fixture.
    fn new(trace: &'a str, expectation: &'a TraceExpectation) -> Self {
        let quoted = format!("\"{}\"", expectation.name);
        let lines: Vec<&'a str> = trace
            .lines()
            .filter(|line| line.contains(&quoted))
            .collect();
        Self {
            name: &expectation.name,
            lines,
            path: expectation.path,
        }
    }

    /// Whether any collected line shows the syscall `shape`.
    fn shows(&self, shape: TraceLine) -> bool {
        self.lines
            .iter()
            .copied()
            .any(|line| line_matches(shape, line))
    }

    /// The collected lines, or a placeholder when the trace never named them.
    fn excerpt(&self) -> String {
        if self.lines.is_empty() {
            "<no trace lines>".to_owned()
        } else {
            self.lines.join("\n")
        }
    }

    /// Assert that the injected fault refused the fixture's `openat2`.
    fn require_injected_refusal(&self) -> Result<()> {
        ensure!(
            self.shows(TraceLine::RefusedOpenat2),
            "the injected fault did not refuse an openat2 for {}:\n{}",
            self.name,
            self.excerpt()
        );
        Ok(())
    }

    /// Assert that the manual resolver then attempted an `openat`.
    fn require_manual_open(&self) -> Result<()> {
        ensure!(
            self.shows(TraceLine::Openat),
            "the manual resolver did not open {} after openat2 was refused:\n{}",
            self.name,
            self.excerpt()
        );
        Ok(())
    }

    /// Assert the outcome this fixture's symlink policy requires.
    fn require_outcome(&self) -> Result<()> {
        match self.path {
            TracePath::RefusedLink => self.require_refused_link(),
            TracePath::FollowedLink => self.require_followed_link(),
            TracePath::RegularFile => self.require_regular_file(),
        }
    }

    /// Assert that the no-follow policy refused the link without resolving it.
    ///
    /// The `readlinkat` check is the one that catches the original defect: a
    /// resolver that consulted cap-std's logical `FollowSymlinks::Yes` default
    /// dereferenced the final component even though `O_NOFOLLOW` was set, and
    /// only that syscall records the dereference.
    fn require_refused_link(&self) -> Result<()> {
        ensure!(
            self.shows(TraceLine::RefusedOpenat),
            "the manual resolver did not refuse the link {} with ELOOP:\n{}",
            self.name,
            self.excerpt()
        );
        ensure!(
            !self.shows(TraceLine::Readlinkat),
            "the manual resolver dereferenced the refused link {}:\n{}",
            self.name,
            self.excerpt()
        );
        Ok(())
    }

    /// Assert that the opt-in resolved the link through `readlinkat`.
    fn require_followed_link(&self) -> Result<()> {
        ensure!(
            self.shows(TraceLine::Readlinkat),
            "the opt-in did not dereference the link {}:\n{}",
            self.name,
            self.excerpt()
        );
        Ok(())
    }

    /// Assert that an ordinary file was opened without an error.
    fn require_regular_file(&self) -> Result<()> {
        ensure!(
            self.shows(TraceLine::SuccessfulOpenat),
            "the manual resolver did not open the regular file {}:\n{}",
            self.name,
            self.excerpt()
        );
        Ok(())
    }
}

/// Assert that `trace` shows the manual resolver handling `expectation`.
pub(super) fn assert_trace(trace: &str, expectation: &TraceExpectation) -> Result<()> {
    let window = TraceWindow::new(trace, expectation);
    window.require_injected_refusal()?;
    window.require_manual_open()?;
    window.require_outcome()
}
