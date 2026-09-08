//! Defines bounded resource accounting for one manifest evaluation.
//!
//! The manifest loader evaluates untrusted template code before it executes a
//! build command.  This module gives every such evaluation one shared budget:
//! the template adapter charges engine-observable fuel, source, and output,
//! while expansion charges iterator and cloned-entry work that `MiniJinja`
//! cannot observe.

use anyhow::Result;
use std::sync::{Arc, Mutex, MutexGuard};

mod types;
mod writer;
pub use types::ManifestBudgetLimits;
pub(crate) use types::{ManifestBudgetExhaustion, ManifestBudgetKind, ManifestBudgetStage};
pub(crate) use writer::CappedWriter;

/// Hold mutable resource counters behind a shared manifest-local handle.
#[derive(Clone, Debug)]
pub(crate) struct ManifestBudget {
    /// Retains immutable configured ceilings for deterministic diagnostics.
    limits: ManifestBudgetLimits,
    /// Shares mutable counter state without widening loader signatures to `&mut`.
    state: Arc<Mutex<ManifestBudgetState>>,
}

/// Store the remaining aggregate counters for one manifest.
#[derive(Debug)]
struct ManifestBudgetState {
    /// Tracks remaining rendered output bytes.
    rendered_bytes: usize,
    /// Tracks remaining template and macro-import source bytes.
    source_bytes: usize,
    /// Tracks remaining expanded target and action entries.
    expanded_entries: usize,
    /// Tracks fuel reserved by active or completed evaluations.
    fuel: u64,
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
            state: Arc::new(Mutex::new(ManifestBudgetState {
                rendered_bytes: validated_limits.rendered_manifest_bytes,
                source_bytes: validated_limits.source_bytes,
                expanded_entries: validated_limits.expanded_entries,
                fuel: validated_limits.manifest_fuel,
            })),
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
            &mut self.lock_state().source_bytes,
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
        let mut counters = self.lock_state();
        let remaining = counters.fuel;
        if remaining == 0 {
            return Err(Self::exhaustion(
                ManifestBudgetKind::Fuel,
                stage,
                self.limits.manifest_fuel,
            ));
        }
        let reserved = remaining.min(self.limits.evaluation_fuel);
        counters.fuel = remaining - reserved;
        Ok(reserved)
    }

    /// Return unused fuel after a state-reporting template evaluation.
    pub(crate) fn refund_unused_fuel(&self, unused: u64) {
        let mut state = self.lock_state();
        state.fuel = state.fuel.saturating_add(unused);
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
            &mut self.lock_state().expanded_entries,
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

    /// Charge a macro result after `MiniJinja` materializes its string value.
    pub(crate) fn charge_macro_output(
        &self,
        bytes: usize,
    ) -> std::result::Result<(), ManifestBudgetExhaustion> {
        if bytes > self.limits.rendered_value_bytes {
            return Err(Self::exhaustion(
                ManifestBudgetKind::ValueBytes,
                ManifestBudgetStage::Macro,
                self.limits.rendered_value_bytes as u64,
            ));
        }
        self.charge_rendered_bytes(bytes)
    }

    /// Charge an aggregate output write after its per-value check succeeds.
    fn charge_rendered_bytes(
        &self,
        bytes: usize,
    ) -> std::result::Result<(), ManifestBudgetExhaustion> {
        Self::charge_remaining(
            &mut self.lock_state().rendered_bytes,
            bytes,
            Self::exhaustion(
                ManifestBudgetKind::RenderedBytes,
                ManifestBudgetStage::ByteAggregate,
                self.limits.rendered_manifest_bytes as u64,
            ),
        )
    }

    /// Charge one remaining counter without underflow.
    const fn charge_remaining(
        remaining: &mut usize,
        amount: usize,
        exhaustion: ManifestBudgetExhaustion,
    ) -> std::result::Result<(), ManifestBudgetExhaustion> {
        let available = *remaining;
        if amount > available {
            return Err(exhaustion);
        }
        *remaining = available - amount;
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

    /// Lock manifest-local accounting, recovering safely from a prior panic.
    fn lock_state(&self) -> MutexGuard<'_, ManifestBudgetState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl Default for ManifestBudget {
    /// Construct fresh accounting from the safe production defaults.
    fn default() -> Self {
        let limits = ManifestBudgetLimits::default();
        Self {
            state: Arc::new(Mutex::new(ManifestBudgetState {
                rendered_bytes: limits.rendered_manifest_bytes,
                source_bytes: limits.source_bytes,
                expanded_entries: limits.expanded_entries,
                fuel: limits.manifest_fuel,
            })),
            limits,
        }
    }
}
