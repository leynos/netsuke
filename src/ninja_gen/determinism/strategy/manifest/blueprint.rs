//! Turning drawn values into a well-formed manifest specification.
//!
//! This is the resolution half of the generator: [`Blueprint`] holds the raw
//! strategy output, and [`Blueprint::build`] constructs the rules, resolves
//! each target's rule reference and dependency slots, and derives the declared
//! defaults. The split from the parent module is the file-size cap rather than
//! a boundary in the reasoning; the well-formedness argument lives in the
//! parent module, because the constraints belong with the domains they define.

use super::super::graph::rotate;
use super::{
    ManifestSpec, RulePlan, TargetPlan, deps_for, ensure_cross_target_dependency, rule_name_for,
    rule_plan,
};

/// The drawn values, before any path or rule is resolved.
pub(super) struct Blueprint {
    /// Number of targets to declare.
    pub(super) target_count: usize,
    /// Number of rules to declare.
    pub(super) rule_count: usize,
    /// Rule index drawn for each target slot.
    pub(super) rule_picks: Vec<usize>,
    /// Dependency slot picks drawn for each target slot.
    pub(super) dep_picks: Vec<Vec<usize>>,
    /// Serial-order flag drawn for each target slot.
    pub(super) serial_flags: Vec<bool>,
    /// Default slot picks, before they are restricted to declared slots.
    pub(super) default_picks: Vec<usize>,
    /// One permutation of every target slot.
    pub(super) shuffled: Vec<usize>,
    /// The rotation that derives the second declaration order.
    pub(super) rotation: usize,
}

impl Blueprint {
    /// Resolve the drawn values into a specification.
    pub(super) fn build(self) -> ManifestSpec {
        let rules = self.rules();
        let targets = self.targets(&rules);
        let declared_defaults = self.declared_defaults();
        let second_defaults = rotate(&declared_defaults, self.rotation);
        ManifestSpec {
            rules,
            targets,
            declared_defaults,
            second_targets: rotate(&self.shuffled, self.rotation),
            first_targets: self.shuffled,
            second_defaults,
        }
    }

    /// The specification's rules, named by index.
    fn rules(&self) -> Vec<RulePlan> {
        (0..self.rule_count).map(rule_plan).collect()
    }

    /// The specification's targets, one per drawn slot.
    fn targets(&self, rules: &[RulePlan]) -> Vec<TargetPlan> {
        let mut targets: Vec<TargetPlan> = (0..self.target_count)
            .map(|slot| TargetPlan {
                slot,
                rule_name: rule_name_for(slot, self.rule_picks.get(slot).copied(), rules),
                deps: deps_for(slot, self.dep_picks.get(slot).map(Vec::as_slice)),
                is_serial: self.serial_flags.get(slot).copied().unwrap_or(false),
            })
            .collect();
        ensure_cross_target_dependency(&mut targets);
        targets
    }

    /// The declared defaults: distinct target slots in descending order.
    ///
    /// Descending is the obligation's precondition, not a stylistic choice. The
    /// emitter sorts this list, so a declaration that was already ascending
    /// would be emitted unchanged even with the sort deleted and `MUT-DEFSORT`
    /// could pass here vacuously. A descending declaration of two or more
    /// distinct targets is never already ascending, which is the class the
    /// coverage tally counts.
    ///
    /// The drawn picks are restricted to slots below `target_count` and
    /// deduplicated, so a case never names a target it does not declare. An
    /// empty result falls back to the last slot, keeping the declaration
    /// non-empty as the domain requires.
    fn declared_defaults(&self) -> Vec<usize> {
        let mut defaults: Vec<usize> = self
            .default_picks
            .iter()
            .copied()
            .filter(|slot| *slot < self.target_count)
            .collect();
        defaults.sort_unstable();
        defaults.dedup();
        defaults.reverse();
        if defaults.is_empty() {
            defaults.push(self.target_count.saturating_sub(1));
        }
        defaults
    }
}
