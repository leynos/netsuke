//! Defines bounded resource accounting for one manifest evaluation.
//!
//! The manifest loader evaluates untrusted template code before it executes a
//! build command.  This module gives every such evaluation one shared budget:
//! the template adapter charges engine-observable fuel, source, and output,
//! while expansion charges iterator and cloned-entry work that `MiniJinja`
//! cannot observe.

use anyhow::Result;
use std::sync::{Arc, Mutex, MutexGuard};

pub(in crate::manifest) mod adapter;
mod allowance;
mod types;
mod writer;
pub(crate) use allowance::CollectionAllowance;
pub use types::ManifestBudgetLimits;
pub(crate) use types::{
    DEFAULT_COLLECTION_CALL_ITEMS, DEFAULT_COLLECTION_ITEMS, ManifestBudgetExhaustion,
    ManifestBudgetKind, ManifestBudgetStage,
};
pub(crate) use writer::CappedWriter;

/// Hold mutable resource counters behind a shared manifest-local handle.
#[derive(Clone, Debug)]
pub(crate) struct ManifestBudget {
    /// Retains immutable configured ceilings for deterministic diagnostics.
    limits: ManifestBudgetLimits,
    /// Caps members consumed by one native collection filter call.
    collection_call_items: usize,
    /// Caps members consumed by native collection filters across the manifest.
    collection_items_limit: usize,
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
    /// Tracks remaining native collection-filter members.
    collection_items: usize,
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
                collection_items: DEFAULT_COLLECTION_ITEMS,
            })),
            collection_call_items: DEFAULT_COLLECTION_CALL_ITEMS,
            collection_items_limit: DEFAULT_COLLECTION_ITEMS,
            limits: validated_limits,
        })
    }

    /// Construct accounting with crate-internal collection ceilings for tests.
    ///
    /// The public limit type deliberately does not expose these ceilings: the
    /// collection bound is a fixed contract rather than an operator setting.
    /// Tests still need small values, so this constructor replaces both without
    /// widening the public configuration surface.
    ///
    /// # Errors
    ///
    /// Returns an error when any configured public limit is zero.
    #[cfg(test)]
    pub(crate) fn with_collection_limits(
        limits: ManifestBudgetLimits,
        collection_call_items: usize,
        collection_items: usize,
    ) -> Result<Self> {
        let mut budget = Self::new(limits)?;
        budget.collection_call_items = collection_call_items;
        budget.collection_items_limit = collection_items;
        budget.lock_state().collection_items = collection_items;
        Ok(budget)
    }

    /// Borrow this manifest's collection allowance for one registration.
    pub(crate) fn collection_allowance(&self) -> CollectionAllowance {
        CollectionAllowance::Manifest(self.clone())
    }

    /// Check one collection-filter member against the per-call ceiling.
    pub(crate) const fn check_collection_cardinality(
        &self,
        index: usize,
    ) -> std::result::Result<(), ManifestBudgetExhaustion> {
        if index < self.collection_call_items {
            Ok(())
        } else {
            Err(Self::exhaustion(
                ManifestBudgetKind::CollectionCallItems,
                ManifestBudgetStage::Collection,
                self.collection_call_items as u64,
            ))
        }
    }

    /// Charge one collection-filter member against the manifest aggregate.
    pub(crate) fn charge_collection_item(
        &self,
    ) -> std::result::Result<(), ManifestBudgetExhaustion> {
        Self::charge_remaining(
            &mut self.lock_state().collection_items,
            1,
            Self::exhaustion(
                ManifestBudgetKind::CollectionItems,
                ManifestBudgetStage::CollectionAggregate,
                self.collection_items_limit as u64,
            ),
        )
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
                collection_items: DEFAULT_COLLECTION_ITEMS,
            })),
            collection_call_items: DEFAULT_COLLECTION_CALL_ITEMS,
            collection_items_limit: DEFAULT_COLLECTION_ITEMS,
            limits,
        }
    }
}

#[cfg(test)]
mod tests {
    //! Verify isolated aggregate accounting boundaries.

    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::exact_limit(8, true)]
    #[case::one_byte_over(9, false)]
    fn source_byte_limit_accepts_the_exact_allowance_and_rejects_one_more(
        #[case] requested_bytes: usize,
        #[case] should_succeed: bool,
    ) {
        const LIMIT: usize = 8;
        let budget = ManifestBudget::new(ManifestBudgetLimits {
            source_bytes: LIMIT,
            ..ManifestBudgetLimits::default()
        })
        .expect("positive default limits plus a positive source limit must validate");
        let result = budget.charge_source(requested_bytes, ManifestBudgetStage::Source);

        if should_succeed {
            result.expect("the exact source limit must succeed");
        } else {
            let exhaustion = result.expect_err("one byte over the source limit must fail");
            assert_eq!(exhaustion.kind, ManifestBudgetKind::SourceBytes);
            assert_eq!(exhaustion.stage, ManifestBudgetStage::Source);
            assert_eq!(exhaustion.limit, LIMIT as u64);
        }
    }
}
