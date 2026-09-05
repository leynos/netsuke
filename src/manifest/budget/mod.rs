//! Defines bounded resource accounting for one manifest evaluation.
//!
//! The manifest loader evaluates untrusted template code before it executes a
//! build command.  This module gives every such evaluation one shared budget:
//! the template adapter charges engine-observable fuel, source, and output,
//! while expansion charges iterator and cloned-entry work that `MiniJinja`
//! cannot observe.

use crate::{localization, localization::keys, manifest::jinja_macros::telemetry};
use anyhow::{Result, ensure};
use minijinja::{Error, ErrorKind};
use std::{cell::Cell, rc::Rc};

mod writer;
pub(crate) use writer::CappedWriter;

/// Default maximum `MiniJinja` instructions reserved for one evaluation.
pub const DEFAULT_EVALUATION_FUEL: u64 = 1_000_000;
/// Default maximum bytes produced by one rendered manifest value.
pub const DEFAULT_RENDERED_VALUE_BYTES: usize = 1_048_576;
/// Default maximum rendered bytes produced by one manifest.
pub const DEFAULT_RENDERED_MANIFEST_BYTES: usize = 16_777_216;
/// Default maximum source bytes consumed by templates and macro imports.
pub const DEFAULT_SOURCE_BYTES: usize = 4_194_304;
/// Default maximum items consumed by one `foreach` expression.
pub const DEFAULT_FOREACH_CARDINALITY: usize = 10_000;
/// Default maximum targets and actions emitted by expansion.
pub const DEFAULT_EXPANDED_ENTRIES: usize = 50_000;
/// Default maximum `MiniJinja` instructions reserved across one manifest.
pub const DEFAULT_MANIFEST_FUEL: u64 = 100_000_000;

/// Configure the resource ceilings applied to a single manifest evaluation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ManifestBudgetLimits {
    /// Caps fuel reserved for one render or expression evaluation.
    pub evaluation_fuel: u64,
    /// Caps the bytes emitted by one rendered string value.
    pub rendered_value_bytes: usize,
    /// Caps all rendered bytes emitted while loading one manifest.
    pub rendered_manifest_bytes: usize,
    /// Caps all template and generated macro-import source bytes.
    pub source_bytes: usize,
    /// Caps the values consumed from one `foreach` iterator.
    pub foreach_cardinality: usize,
    /// Caps target and action entries emitted by expansion.
    pub expanded_entries: usize,
    /// Caps fuel reserved across all manifest evaluations.
    pub manifest_fuel: u64,
}

impl Default for ManifestBudgetLimits {
    /// Return the safe production resource ceilings.
    fn default() -> Self {
        Self {
            evaluation_fuel: DEFAULT_EVALUATION_FUEL,
            rendered_value_bytes: DEFAULT_RENDERED_VALUE_BYTES,
            rendered_manifest_bytes: DEFAULT_RENDERED_MANIFEST_BYTES,
            source_bytes: DEFAULT_SOURCE_BYTES,
            foreach_cardinality: DEFAULT_FOREACH_CARDINALITY,
            expanded_entries: DEFAULT_EXPANDED_ENTRIES,
            manifest_fuel: DEFAULT_MANIFEST_FUEL,
        }
    }
}

impl ManifestBudgetLimits {
    /// Validate that every configured resource ceiling is positive.
    ///
    /// # Errors
    ///
    /// Returns an error when an operator configures a zero ceiling.
    pub fn validate(self) -> Result<Self> {
        ensure!(
            self.evaluation_fuel > 0,
            "manifest evaluation fuel must be positive"
        );
        ensure!(
            self.rendered_value_bytes > 0,
            "manifest rendered value bytes must be positive"
        );
        ensure!(
            self.rendered_manifest_bytes > 0,
            "manifest rendered bytes must be positive"
        );
        ensure!(
            self.source_bytes > 0,
            "manifest source bytes must be positive"
        );
        ensure!(
            self.foreach_cardinality > 0,
            "manifest foreach cardinality must be positive"
        );
        ensure!(
            self.expanded_entries > 0,
            "manifest expanded entries must be positive"
        );
        ensure!(self.manifest_fuel > 0, "manifest fuel must be positive");
        Ok(self)
    }
}

/// Name the closed-vocabulary stage at which manifest work is charged.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ManifestBudgetStage {
    /// Render a normal manifest field.
    Render,
    /// Consume a `foreach` iterator.
    Foreach,
    /// Evaluate a `when` condition.
    When,
    /// Invoke a manifest macro.
    Macro,
    /// Clone an expansion entry.
    ExpansionAggregate,
    /// Account aggregate rendered output.
    ByteAggregate,
    /// Parse template and macro-import source.
    Source,
    /// Execute `MiniJinja` instructions.
    Fuel,
}

impl ManifestBudgetStage {
    /// Return every permitted telemetry stage in its stable vocabulary.
    pub(crate) const fn all() -> [Self; 8] {
        [
            Self::Render,
            Self::Foreach,
            Self::When,
            Self::Macro,
            Self::ExpansionAggregate,
            Self::ByteAggregate,
            Self::Source,
            Self::Fuel,
        ]
    }

    /// Return the stable telemetry and diagnostic stage label.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Render => "render",
            Self::Foreach => "foreach",
            Self::When => "when",
            Self::Macro => "macro",
            Self::ExpansionAggregate => "expansion_aggregate",
            Self::ByteAggregate => "byte_aggregate",
            Self::Source => "source",
            Self::Fuel => "fuel",
        }
    }
}

/// Name the resource counter exhausted by an operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ManifestBudgetKind {
    /// Counts `MiniJinja` instructions.
    Fuel,
    /// Counts bytes emitted by one rendered value.
    ValueBytes,
    /// Counts aggregate rendered bytes.
    RenderedBytes,
    /// Counts template and generated import source bytes.
    SourceBytes,
    /// Counts values consumed by one `foreach` expression.
    ForeachCardinality,
    /// Counts target and action entries emitted by expansion.
    ExpandedEntries,
}

impl ManifestBudgetKind {
    /// Return the stable telemetry budget label.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Fuel => "fuel",
            Self::ValueBytes => "value_bytes",
            Self::RenderedBytes => "rendered_bytes",
            Self::SourceBytes => "source_bytes",
            Self::ForeachCardinality => "foreach_cardinality",
            Self::ExpandedEntries => "expanded_entries",
        }
    }
}

/// Describe one deterministic resource-budget exhaustion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ManifestBudgetExhaustion {
    /// Identifies the exhausted counter.
    pub(crate) kind: ManifestBudgetKind,
    /// Identifies the evaluation stage that attempted the charge.
    pub(crate) stage: ManifestBudgetStage,
    /// Supplies the configured limit without manifest-controlled data.
    pub(crate) limit: u64,
}

impl ManifestBudgetExhaustion {
    /// Convert the exhaustion into the localized `MiniJinja` diagnostic.
    pub(crate) fn into_error(self, kind: ErrorKind) -> Error {
        telemetry::record_budget_exhaustion(self.stage.as_str(), self.kind.as_str());
        Error::new(
            kind,
            localization::message(keys::MANIFEST_BUDGET_EXCEEDED)
                .with_arg("stage", self.stage.as_str())
                .with_arg("limit", self.limit)
                .to_string(),
        )
    }
}

/// Hold mutable resource counters behind a shared manifest-local handle.
#[derive(Clone, Debug)]
pub(crate) struct ManifestBudget {
    /// Retains immutable configured ceilings for deterministic diagnostics.
    limits: ManifestBudgetLimits,
    /// Shares mutable counter state without widening loader signatures to `&mut`.
    state: Rc<ManifestBudgetState>,
}

/// Store the remaining aggregate counters for one manifest.
#[derive(Debug)]
struct ManifestBudgetState {
    /// Tracks remaining rendered output bytes.
    rendered_bytes: Cell<usize>,
    /// Tracks remaining template and macro-import source bytes.
    source_bytes: Cell<usize>,
    /// Tracks remaining expanded target and action entries.
    expanded_entries: Cell<usize>,
    /// Tracks fuel reserved by active or completed evaluations.
    fuel: Cell<u64>,
}

impl ManifestBudget {
    /// Construct fresh runtime accounting for validated manifest limits.
    ///
    /// # Errors
    ///
    /// Returns an error when any configured limit is zero.
    pub(crate) fn new(limits: ManifestBudgetLimits) -> Result<Self> {
        let validated_limits = limits.validate()?;
        Ok(Self {
            state: Rc::new(ManifestBudgetState {
                rendered_bytes: Cell::new(validated_limits.rendered_manifest_bytes),
                source_bytes: Cell::new(validated_limits.source_bytes),
                expanded_entries: Cell::new(validated_limits.expanded_entries),
                fuel: Cell::new(validated_limits.manifest_fuel),
            }),
            limits: validated_limits,
        })
    }

    /// Charge template source bytes before `MiniJinja` parses them.
    pub(crate) fn charge_source(
        &self,
        bytes: usize,
        stage: ManifestBudgetStage,
    ) -> std::result::Result<(), ManifestBudgetExhaustion> {
        Self::charge_remaining(
            &self.state.source_bytes,
            bytes,
            Self::exhaustion(
                ManifestBudgetKind::SourceBytes,
                stage,
                self.limits.source_bytes as u64,
            ),
        )
    }

    /// Reserve a fuel cap for one evaluation, returning the engine allowance.
    pub(crate) fn reserve_fuel(
        &self,
        stage: ManifestBudgetStage,
    ) -> std::result::Result<u64, ManifestBudgetExhaustion> {
        let remaining = self.state.fuel.get();
        if remaining == 0 {
            return Err(Self::exhaustion(
                ManifestBudgetKind::Fuel,
                stage,
                self.limits.manifest_fuel,
            ));
        }
        let reserved = remaining.min(self.limits.evaluation_fuel);
        self.state.fuel.set(remaining - reserved);
        Ok(reserved)
    }

    /// Return unused fuel after a state-reporting template evaluation.
    pub(crate) fn refund_unused_fuel(&self, unused: u64) {
        self.state
            .fuel
            .set(self.state.fuel.get().saturating_add(unused));
    }

    /// Describe fuel exhaustion for an engine evaluation that consumed its reservation.
    pub(crate) const fn fuel_exhaustion(
        &self,
        stage: ManifestBudgetStage,
    ) -> ManifestBudgetExhaustion {
        Self::exhaustion(ManifestBudgetKind::Fuel, stage, self.limits.evaluation_fuel)
    }

    /// Charge one expanded target or action before cloning its map.
    pub(crate) fn charge_expanded_entry(
        &self,
        stage: ManifestBudgetStage,
    ) -> std::result::Result<(), ManifestBudgetExhaustion> {
        Self::charge_remaining(
            &self.state.expanded_entries,
            1,
            Self::exhaustion(
                ManifestBudgetKind::ExpandedEntries,
                stage,
                self.limits.expanded_entries as u64,
            ),
        )
    }

    /// Check one iterator position against the per-`foreach` ceiling.
    pub(crate) const fn check_foreach_cardinality(
        &self,
        index: usize,
    ) -> std::result::Result<(), ManifestBudgetExhaustion> {
        if index < self.limits.foreach_cardinality {
            Ok(())
        } else {
            Err(Self::exhaustion(
                ManifestBudgetKind::ForeachCardinality,
                ManifestBudgetStage::Foreach,
                self.limits.foreach_cardinality as u64,
            ))
        }
    }

    /// Construct a writer that bounds one rendered string and shared output.
    pub(crate) const fn capped_writer(&self) -> CappedWriter<'_> {
        CappedWriter::new(self)
    }

    /// Charge an aggregate output write after its per-value check succeeds.
    fn charge_rendered_bytes(
        &self,
        bytes: usize,
    ) -> std::result::Result<(), ManifestBudgetExhaustion> {
        Self::charge_remaining(
            &self.state.rendered_bytes,
            bytes,
            Self::exhaustion(
                ManifestBudgetKind::RenderedBytes,
                ManifestBudgetStage::ByteAggregate,
                self.limits.rendered_manifest_bytes as u64,
            ),
        )
    }

    /// Charge one cell-backed remaining counter without underflow.
    fn charge_remaining(
        remaining: &Cell<usize>,
        amount: usize,
        exhaustion: ManifestBudgetExhaustion,
    ) -> std::result::Result<(), ManifestBudgetExhaustion> {
        let available = remaining.get();
        if amount > available {
            return Err(exhaustion);
        }
        remaining.set(available - amount);
        Ok(())
    }

    /// Build a deterministic exhaustion without including manifest data.
    const fn exhaustion(
        kind: ManifestBudgetKind,
        stage: ManifestBudgetStage,
        limit: u64,
    ) -> ManifestBudgetExhaustion {
        ManifestBudgetExhaustion { kind, stage, limit }
    }
}

impl Default for ManifestBudget {
    /// Construct fresh accounting from the safe production defaults.
    fn default() -> Self {
        let limits = ManifestBudgetLimits::default();
        Self {
            state: Rc::new(ManifestBudgetState {
                rendered_bytes: Cell::new(limits.rendered_manifest_bytes),
                source_bytes: Cell::new(limits.source_bytes),
                expanded_entries: Cell::new(limits.expanded_entries),
                fuel: Cell::new(limits.manifest_fuel),
            }),
            limits,
        }
    }
}
