//! Bound native collection-filter consumption for one registration.
//!
//! `MiniJinja` fuel meters engine instructions, not the items a native callback
//! pulls from a lazy iterator. A short expression such as `[none] * n` can name
//! an enormous logical input, so a filter that collects first can spend
//! unbounded CPU and memory before the engine resumes. This allowance charges
//! one unit per consumed member and lets the filter stop at its configured
//! ceiling.
//!
//! Manifest loads share an allowance with the manifest's other accounting;
//! standalone stdlib registration enforces the per-call ceiling alone, because
//! no manifest evaluation exists to charge.

use super::{
    DEFAULT_COLLECTION_CALL_ITEMS, ManifestBudget, ManifestBudgetExhaustion, ManifestBudgetKind,
    ManifestBudgetStage, adapter::BudgetErrorExt,
};
use minijinja::{Error, ErrorKind};

/// Bound the members a native collection filter may consume.
#[derive(Clone, Debug)]
pub(crate) enum CollectionAllowance {
    /// Manifest mode: a per-call ceiling and a manifest-wide aggregate.
    Manifest(ManifestBudget),
    /// Standalone mode: the per-call ceiling alone, with no shared state.
    Standalone {
        /// Caps members consumed by one filter call.
        call_items: usize,
    },
}

impl CollectionAllowance {
    /// Build an allowance for a registration with no manifest budget.
    pub(crate) const fn standalone() -> Self {
        Self::Standalone {
            call_items: DEFAULT_COLLECTION_CALL_ITEMS,
        }
    }

    /// Charge one member before the filter examines or retains it.
    ///
    /// Takes the zero-based member index so the per-call ceiling is checked
    /// without shared state, exactly as `foreach` cardinality is. Charging
    /// before the blank check is what stops an all-blank repetition; charging
    /// before the append is what stops the retained vector outgrowing its
    /// ceiling.
    ///
    /// # Errors
    ///
    /// Returns an `OutOfFuel` `MiniJinja` error carrying typed budget
    /// exhaustion when the call allowance or the manifest aggregate is spent.
    pub(crate) fn charge_member(&self, index: usize) -> Result<(), Error> {
        self.check_member(index)
            .map_err(|exhaustion| exhaustion.into_error(ErrorKind::OutOfFuel))
    }

    /// Classify one member charge without adapting it to a `MiniJinja` error.
    fn check_member(&self, index: usize) -> Result<(), ManifestBudgetExhaustion> {
        match self {
            Self::Manifest(budget) => {
                budget.check_collection_cardinality(index)?;
                budget.charge_collection_item()
            }
            Self::Standalone { call_items } => check_call_index(index, *call_items),
        }
    }
}

/// Check one zero-based member index against a per-call ceiling.
const fn check_call_index(index: usize, limit: usize) -> Result<(), ManifestBudgetExhaustion> {
    if index < limit {
        Ok(())
    } else {
        Err(ManifestBudgetExhaustion {
            kind: ManifestBudgetKind::CollectionCallItems,
            stage: ManifestBudgetStage::Collection,
            limit: limit as u64,
        })
    }
}

#[cfg(test)]
mod tests {
    //! Verify call and aggregate boundaries, shared clones, and diagnostics.

    use super::{
        CollectionAllowance, ManifestBudget, ManifestBudgetExhaustion, ManifestBudgetKind,
        ManifestBudgetStage,
    };
    use crate::manifest::ManifestBudgetLimits;
    use minijinja::{Error, ErrorKind};
    use rstest::rstest;
    use test_support::{EnLocalizer, en_localizer, fluent::normalize_fluent_isolates};

    /// Build manifest accounting with small crate-internal collection ceilings.
    fn budget_with(collection_call_items: usize, collection_items: usize) -> ManifestBudget {
        ManifestBudget::with_collection_limits(
            ManifestBudgetLimits::default(),
            collection_call_items,
            collection_items,
        )
        .expect("default public limits must construct manifest accounting")
    }

    /// Find the typed exhaustion beneath an adapted `MiniJinja` error.
    fn exhaustion_of(error: &Error) -> &ManifestBudgetExhaustion {
        let mut source = std::error::Error::source(error);
        while let Some(current) = source {
            if let Some(exhaustion) = current.downcast_ref::<ManifestBudgetExhaustion>() {
                return exhaustion;
            }
            source = current.source();
        }
        panic!("the adapted error must carry typed budget exhaustion: {error}");
    }

    /// Charge every index below `limit` and assert each charge succeeds.
    fn charge_all_below(allowance: &CollectionAllowance, limit: usize) {
        for index in 0..limit {
            allowance
                .charge_member(index)
                .expect("every index below the call ceiling must fit the allowance");
        }
    }

    #[test]
    fn call_limit_accepts_the_exact_allowance_and_rejects_one_more() {
        let allowance = CollectionAllowance::Manifest(budget_with(4, 8));
        charge_all_below(&allowance, 4);

        let error = allowance
            .charge_member(4)
            .expect_err("the fifth member must exceed a four-member call ceiling");
        assert_eq!(
            error.kind(),
            ErrorKind::OutOfFuel,
            "the adapted error must use the fuel kind so engine paths preserve it"
        );
        let exhaustion = exhaustion_of(&error);
        assert_eq!(
            exhaustion.kind,
            ManifestBudgetKind::CollectionCallItems,
            "the call ceiling must report its own counter"
        );
        assert_eq!(exhaustion.stage, ManifestBudgetStage::Collection);
        assert_eq!(exhaustion.limit, 4);
    }

    #[test]
    fn clones_share_the_manifest_aggregate() {
        let budget = budget_with(4, 6);
        let first = budget.collection_allowance();
        let second = budget.collection_allowance();
        charge_all_below(&first, 4);
        charge_all_below(&second, 2);

        let error = second
            .charge_member(2)
            .expect_err("the seventh aggregate member must exhaust the six-member allowance");
        let exhaustion = exhaustion_of(&error);
        assert_eq!(exhaustion.kind, ManifestBudgetKind::CollectionItems);
        assert_eq!(exhaustion.stage, ManifestBudgetStage::CollectionAggregate);
        assert_eq!(exhaustion.limit, 6);
    }

    #[test]
    fn a_fresh_budget_restores_the_aggregate() {
        let first = budget_with(4, 1).collection_allowance();
        first
            .charge_member(0)
            .expect("the first manifest must spend its single member");

        let second = budget_with(4, 1).collection_allowance();
        second
            .charge_member(0)
            .expect("a fresh manifest must have its own aggregate");
    }

    #[rstest]
    fn adapted_error_reports_only_the_stage_and_limit(en_localizer: EnLocalizer) {
        let _en = en_localizer;
        let allowance = CollectionAllowance::Manifest(budget_with(4, 8));
        let error = allowance
            .charge_member(4)
            .expect_err("the fifth member must exceed the call ceiling");

        let rendered = normalize_fluent_isolates(&error.to_string());
        assert!(
            rendered.contains("during collection"),
            "the diagnostic must name the fixed stage: {rendered}"
        );
        assert!(
            rendered.contains('4'),
            "the diagnostic must name the numeric limit: {rendered}"
        );
        // The exhaustion carries only a stage, a counter, and a limit, so the
        // rendered message is exactly the localized template with those two
        // arguments. Any additional content would have to come from somewhere
        // the type cannot hold.
        assert!(
            rendered.starts_with("engine ran out of fuel: Manifest resource budget exhausted"),
            "the diagnostic must be the localized template and nothing else: {rendered}"
        );
    }

    #[test]
    fn standalone_allowance_enforces_the_call_ceiling_without_an_aggregate() {
        let allowance = CollectionAllowance::Standalone { call_items: 4 };
        charge_all_below(&allowance, 4);
        // A standalone registration has no manifest to accumulate against, so
        // repeating the same call must restart the per-call allowance.
        charge_all_below(&allowance, 4);

        let error = allowance
            .charge_member(4)
            .expect_err("the fifth member must exceed the standalone call ceiling");
        let exhaustion = exhaustion_of(&error);
        assert_eq!(exhaustion.kind, ManifestBudgetKind::CollectionCallItems);
        assert_eq!(exhaustion.stage, ManifestBudgetStage::Collection);
        assert_eq!(exhaustion.limit, 4);
    }
}
