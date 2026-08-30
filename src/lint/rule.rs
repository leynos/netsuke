//! The typed lint rule interface.
//!
//! A rule declares immutable metadata and binds to exactly one compiler stage.
//! It states what it found and where; the engine decides how loudly to say it,
//! so a rule can never contradict the policy a project configured.

use super::document::{Document, Span};
use super::finding::Finding;
use super::severity::{DefaultSeverity, Severity};
use super::suppress::Directive;
use crate::ast::NetsukeManifest;
use crate::ir::BuildGraph;

/// The concern a rule addresses.
///
/// Category is metadata rather than part of a rule's identifier, so that
/// recategorizing a rule cannot invalidate a configuration file or a
/// suppression comment that named it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Category {
    /// The manifest is likely to behave differently from what it says.
    Correctness,
    /// The declaration defeats change detection or forces needless rebuilds.
    Caching,
    /// The construct depends on a shell or platform Netsuke does not promise.
    Portability,
    /// The recipe's result depends on something other than its declared inputs.
    Determinism,
    /// The declaration is unnecessary, inert, or duplicated.
    Redundancy,
    /// The declaration is never used.
    Hygiene,
    /// A canonical alternative reads better or is easier to discover.
    Clarity,
    /// A workaround for behaviour that a released version has since changed.
    Migration,
    /// The lint directives themselves are wrong or stale.
    Suppression,
}

impl Category {
    /// Every category, in the order the rule reference lists them.
    pub const ALL: [Self; 9] = [
        Self::Correctness,
        Self::Caching,
        Self::Portability,
        Self::Determinism,
        Self::Redundancy,
        Self::Hygiene,
        Self::Clarity,
        Self::Migration,
        Self::Suppression,
    ];

    /// Name this category using its selector spelling.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Correctness => "correctness",
            Self::Caching => "caching",
            Self::Portability => "portability",
            Self::Determinism => "determinism",
            Self::Redundancy => "redundancy",
            Self::Hygiene => "hygiene",
            Self::Clarity => "clarity",
            Self::Migration => "migration",
            Self::Suppression => "suppression",
        }
    }

    /// Resolve a category from its selector spelling.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|entry| entry.as_str() == text)
    }
}

/// The compiler artefact a rule inspects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Stage {
    /// The authored source, before expansion and rendering.
    Document,
    /// The expanded and rendered manifest.
    Manifest,
    /// The lowered build graph.
    Graph,
    /// The lint directives themselves, after every other stage has reported.
    Directive,
}

impl Stage {
    /// Name this stage for `--explain` output and the rule reference.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Document => "document",
            Self::Manifest => "manifest",
            Self::Graph => "graph",
            Self::Directive => "directive",
        }
    }
}

/// Immutable description of one lint rule.
///
/// The summary, rationale, and remediation live here for the prototype period
/// so that the emitted diagnostic, the `--explain` output, and the rule
/// reference document cannot drift apart while the rule set is still settling.
/// Roadmap step 7.2 moves that prose into the localization catalogues, keyed by
/// [`RuleMeta::name`], and keeps this text as the source-locale fallback. The
/// identifiers never move: a name, a category, a severity, and a code are
/// values a user types and a machine matches. See
/// `docs/adr-015-manifest-linting-under-netsuke-check.md`.
#[derive(Debug, Clone, Copy)]
pub struct RuleMeta {
    /// Stable kebab-case identifier, unique across every stage.
    pub name: &'static str,
    /// The concern the rule addresses.
    pub category: Category,
    /// The compiler artefact the rule inspects.
    pub stage: Stage,
    /// The severity the rule reports at unless policy overrides it.
    pub default_severity: DefaultSeverity,
    /// One-line description of what the rule detects.
    pub summary: &'static str,
    /// Why the detected construct is a problem.
    pub rationale: &'static str,
    /// The canonical alternative, stated as an instruction.
    pub remediation: &'static str,
}

impl RuleMeta {
    /// Build the `miette` diagnostic code for this rule.
    #[must_use]
    pub fn code(&self) -> String {
        format!("netsuke::lint::{}", self.name.replace('-', "_"))
    }

    /// Build the documentation anchor for this rule in the rule reference.
    #[must_use]
    pub fn doc_url(&self) -> String {
        format!("{RULE_DOC_BASE}#{}", self.name)
    }
}

/// Base URL of the published rule reference, used for each finding's `url`.
pub const RULE_DOC_BASE: &str =
    "https://github.com/leynos/netsuke/blob/main/docs/netsuke-linter-rules.md";

/// A rule that inspects the authored source.
pub trait DocumentRule: Sync {
    /// Describe the rule.
    fn meta(&self) -> &'static RuleMeta;
    /// Inspect `doc` and report findings.
    fn check(&self, doc: &Document, sink: &mut FindingSink<'_>);
}

/// A rule that inspects the expanded and rendered manifest.
pub trait ManifestRule: Sync {
    /// Describe the rule.
    fn meta(&self) -> &'static RuleMeta;
    /// Inspect `ctx` and report findings.
    fn check(&self, ctx: &ManifestContext<'_>, sink: &mut FindingSink<'_>);
}

/// A rule that inspects the lowered build graph.
pub trait GraphRule: Sync {
    /// Describe the rule.
    fn meta(&self) -> &'static RuleMeta;
    /// Inspect `ctx` and report findings.
    fn check(&self, ctx: &GraphContext<'_>, sink: &mut FindingSink<'_>);
}

/// What a manifest-stage rule may inspect.
pub struct ManifestContext<'a> {
    /// The expanded and rendered manifest.
    pub manifest: &'a NetsukeManifest,
    /// The authored source, for span resolution.
    pub document: &'a Document,
}

/// A rule that inspects the lint directives themselves.
pub trait DirectiveRule: Sync {
    /// Describe the rule.
    fn meta(&self) -> &'static RuleMeta;
    /// Inspect `ctx` and report findings.
    fn check(&self, ctx: &DirectiveContext<'_>, sink: &mut FindingSink<'_>);
}

/// What a graph-stage rule may inspect.
pub struct GraphContext<'a> {
    /// The lowered build graph.
    pub graph: &'a BuildGraph,
    /// The expanded manifest the graph was lowered from.
    pub manifest: &'a NetsukeManifest,
    /// The authored source, for span resolution.
    pub document: &'a Document,
}

/// What a directive-stage rule may inspect.
///
/// The usage counts are taken before suppression is applied, so a directive
/// that silenced a finding counts as used even though that finding never
/// reaches the output.
pub struct DirectiveContext<'a> {
    /// Every directive found in the manifest, in source order.
    pub directives: &'a [Directive],
    /// How many stage findings each directive silenced, by directive index.
    pub usage: &'a [usize],
    /// The authored source, for span context.
    pub document: &'a Document,
}

/// Collects findings on behalf of exactly one rule.
///
/// Binding the sink to a rule for the duration of its `check` call is what
/// prevents a rule from attributing a finding to another rule, and is why a
/// rule never names a severity: the engine stamps the resolved policy value.
pub struct FindingSink<'a> {
    /// The rule currently running.
    meta: &'static RuleMeta,
    /// The severity policy resolved for that rule.
    severity: Severity,
    /// Where accepted findings accumulate.
    findings: &'a mut Vec<Finding>,
}

impl<'a> FindingSink<'a> {
    /// Bind a sink to `meta` at the policy-resolved `severity`.
    pub const fn new(
        meta: &'static RuleMeta,
        severity: Severity,
        findings: &'a mut Vec<Finding>,
    ) -> Self {
        Self {
            meta,
            severity,
            findings,
        }
    }

    /// Describe the running rule, for messages that quote their own metadata.
    #[must_use]
    pub const fn meta(&self) -> &'static RuleMeta {
        self.meta
    }

    /// Report a finding anchored at `span`.
    pub fn at(&mut self, span: Span, message: impl Into<String>) {
        self.findings
            .push(Finding::spanned(self.meta, self.severity, message, span));
    }

    /// Report a finding that could not be resolved to a source span.
    ///
    /// `location` names the target, action, or rule the finding concerns, so
    /// the reader can still find it without a line number.
    pub fn detached(&mut self, location: impl Into<String>, message: impl Into<String>) {
        self.findings.push(Finding::detached(
            self.meta,
            self.severity,
            message,
            location,
        ));
    }

    /// Report a finding at `span` when one is known, and detached otherwise.
    pub fn at_or_detached(
        &mut self,
        span: Option<Span>,
        location: impl Into<String>,
        message: impl Into<String>,
    ) {
        match span {
            Some(found) => self.at(found, message),
            None => self.detached(location, message),
        }
    }
}
