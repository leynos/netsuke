//! A bounded, well-formed manifest specification and its declaration orders.
//!
//! `OBL-E2E` is stated over *declaration order*: permuting a manifest's
//! `targets` list and its declared `defaults` list must not change the emitted
//! bundle. A [`NetsukeManifest`] cannot express that on its own — it derives no
//! `Clone`, so reordering one in place is impossible — and rebuilding one by
//! hand inside the property would put the permutation logic under test inside
//! the test. [`ManifestSpec`] therefore holds the manifest's parts unresolved
//! and rebuilds a fresh value for each declaration order; the property then
//! drives two genuinely different inputs through the same lowering.
//!
//! Well-formedness is established by construction rather than by filtering.
//! Rejection sampling would spend the case budget discarding its own output,
//! and a generated manifest that failed to lower would report a property
//! failure that says nothing about determinism. Four rules carry the
//! guarantee:
//!
//! - Rule names are indexed, so they are pairwise distinct. That is the
//!   obligation's precondition: `process_rules` is last-writer-wins, so two
//!   same-named rules make declaration order observable. The directed witness
//!   in `property_tests::declaration` pins that excluded class.
//! - Output names are indexed (`out/NNNN`) and allocated once, so no two
//!   targets can claim the same output.
//! - A target's dependencies are drawn only from strictly lower slots, so the
//!   graph is acyclic by construction and every dependency resolves to another
//!   target rather than to a missing external file.
//! - The default declaration is a descending list of distinct target slots, so
//!   it is *not already ascending*: deleting the emitter's sort must change
//!   the emitted line, which is what makes `MUT-DEFSORT` observable here.
//!
//! [`blueprint`] holds the resolution step that turns drawn values into a
//! specification; the split is the file-size cap, not a boundary in the
//! reasoning.

use crate::ast::{
    CommandIsolation, DependencyOrder, NetsukeManifest, Recipe, Rule, StringOrList, Target, Vars,
};
use crate::ir::OUTS_TOKEN;
use blueprint::Blueprint;
use proptest::prelude::*;
use semver::Version;
use std::fmt;

pub(super) mod blueprint;

/// Fewest targets a specification declares.
pub(crate) const MIN_TARGETS: usize = 1;
/// Greatest number of targets a specification declares.
pub(crate) const MAX_TARGETS: usize = 30;
/// Fewest rules a specification declares.
pub(crate) const MIN_RULES: usize = 1;
/// Greatest number of rules a specification declares.
pub(crate) const MAX_RULES: usize = 12;
/// Greatest number of dependencies one target declares.
///
/// Three rather than two so that a serial target can exceed one dependency and
/// reach the dyndep-staging branch; the bundle comparison then covers sidecars
/// as well as the build file.
pub(crate) const MAX_DEPS_PER_TARGET: usize = 3;
/// Greatest number of default targets one specification declares.
pub(crate) const MAX_DEFAULT_PICKS: usize = 6;
/// The command the shared rule carries.
///
/// Deliberately free of `{{ outs }}` and `{{ ins }}`: every target referencing
/// the rule expands to the same command, so the lowering's content-hash
/// interning collapses them into one action. A rule body carrying a
/// substitution token would interpolate differently per target and defeat the
/// very class this rule exists to construct.
const SHARED_COMMAND: &str = "echo shared";

/// One rule template, before it is lowered into a [`crate::ast::Rule`].
pub(super) struct RulePlan {
    /// The rule's name, distinct from every other rule's by construction.
    pub(super) name: String,
    /// The command template the rule carries.
    pub(super) command: String,
}

/// One target, before it is lowered into a [`crate::ast::Target`].
pub(super) struct TargetPlan {
    /// The pool slot the target's output is named after.
    pub(super) slot: usize,
    /// The name of the rule this target references.
    pub(super) rule_name: String,
    /// Slots of the lower-indexed targets this target depends on.
    pub(super) deps: Vec<usize>,
    /// Whether the dependency list is declared in serial order.
    pub(super) is_serial: bool,
}

/// A bounded, well-formed manifest and two declaration orders for it.
///
/// Two targets are pinned to the token-free shared rule, so every case with at
/// least two targets exercises action interning. The declared defaults are
/// descending, so every case with at least two defaults exercises the sort.
pub(crate) struct ManifestSpec {
    /// The manifest's rules, in declaration order.
    pub(super) rules: Vec<RulePlan>,
    /// The manifest's targets, indexed by pool slot.
    pub(super) targets: Vec<TargetPlan>,
    /// The declared defaults: distinct slots, descending, never ascending.
    pub(super) declared_defaults: Vec<usize>,
    /// One permutation of every target slot.
    pub(super) first_targets: Vec<usize>,
    /// A second permutation, distinct whenever two targets admit one.
    pub(super) second_targets: Vec<usize>,
    /// A second permutation of the declared defaults.
    pub(super) second_defaults: Vec<usize>,
}

impl ManifestSpec {
    /// Rebuild a fresh manifest declaring the targets and defaults in `order`.
    ///
    /// A fresh value rather than a reordered clone because
    /// [`NetsukeManifest`] derives no `Clone`; [`Rule`], [`Recipe`], and
    /// [`StringOrList`] do, so every part is rebuilt from the specification.
    ///
    /// Out-of-range slots are skipped rather than rejected: shrinking can
    /// shorten a sampled order, and that is not a defect. The generated domain
    /// never produces one.
    #[must_use]
    pub(crate) fn to_manifest(
        &self,
        target_order: &[usize],
        default_order: &[usize],
    ) -> NetsukeManifest {
        NetsukeManifest {
            netsuke_version: Version::new(1, 0, 0),
            vars: Vars::default(),
            macros: Vec::new(),
            rules: self.rules.iter().map(RulePlan::to_rule).collect(),
            actions: Vec::new(),
            targets: target_order
                .iter()
                .filter_map(|slot| self.targets.get(*slot))
                .map(TargetPlan::to_target)
                .collect(),
            defaults: default_order.iter().copied().map(target_name).collect(),
        }
    }

    /// The two target declaration orders this specification compares.
    #[must_use]
    pub(crate) fn target_orders(&self) -> (&[usize], &[usize]) {
        (&self.first_targets, &self.second_targets)
    }

    /// The two default declaration orders this specification compares.
    #[must_use]
    pub(crate) fn default_orders(&self) -> (&[usize], &[usize]) {
        (&self.declared_defaults, &self.second_defaults)
    }
}

impl RulePlan {
    /// Render this plan as a manifest rule.
    fn to_rule(&self) -> Rule {
        Rule {
            name: self.name.clone(),
            recipe: Recipe::Command {
                command: StringOrList::String(self.command.clone()),
                isolation: CommandIsolation::Shared,
            },
            description: None,
        }
    }
}

impl TargetPlan {
    /// Render this plan as a manifest target.
    fn to_target(&self) -> Target {
        Target {
            name: StringOrList::String(target_name(self.slot)),
            recipe: Recipe::Rule {
                rule: StringOrList::String(self.rule_name.clone()),
            },
            sources: StringOrList::Empty,
            deps: dependency_list(&self.deps),
            dependency_order: if self.is_serial {
                DependencyOrder::Serial
            } else {
                DependencyOrder::Parallel
            },
            order_only_deps: StringOrList::Empty,
            vars: Vars::default(),
            phony: false,
            always: false,
            console: false,
            conditional: false,
            description: None,
        }
    }
}

impl fmt::Debug for ManifestSpec {
    /// Print the specification's shape, never its contents.
    ///
    /// Proptest renders a counter-example regardless of the assertion's own
    /// message, and a thirty-target specification is long enough to push the
    /// diagnostic a reader needs out of reach. The counts and the two orders
    /// say which shape a case reached and whether its permutation moved
    /// anything, so a failing case stays diagnosable without replaying its
    /// seed. The alternate flag is ignored for the same reason `GraphSpec`
    /// ignores it: the diagnostic stays one line whatever the caller asks for.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "ManifestSpec {{ targets: {}, rules: {}, defaults: {:?}, first: {:?}, \
             second: {:?} }}",
            self.targets.len(),
            self.rules.len(),
            self.declared_defaults,
            self.first_targets,
            self.second_targets,
        )
    }
}

/// One rule's plan: an indexed name and either the shared or a token-bearing
/// command.
pub(super) fn rule_plan(index: usize) -> RulePlan {
    RulePlan {
        name: format!("rule-{index:02}"),
        command: if index == 0 {
            SHARED_COMMAND.to_owned()
        } else {
            format!("echo rule-{index:02} {OUTS_TOKEN}")
        },
    }
}

/// The rule a target slot references.
///
/// Slots zero and one are pinned to the first rule whatever was drawn. That
/// rule's command carries no substitution token, so both targets expand to the
/// same action and the lowering's content-hash interning collapses them into
/// one identifier — the non-vacuity class `OBL-E2E` names.
pub(super) fn rule_name_for(slot: usize, drawn: Option<usize>, rules: &[RulePlan]) -> String {
    let index = if slot < 2 { 0 } else { drawn.unwrap_or(0) };
    rules
        .get(index)
        .or_else(|| rules.first())
        .map_or_else(String::new, |rule| rule.name.clone())
}

/// The lower-indexed targets one slot depends on.
///
/// Picks at or above the slot are discarded rather than drawn from a
/// slot-bounded range: the bound changes per slot, and a `prop_flat_map` per
/// target would rebuild a strategy for every slot. Discarding is a map rather
/// than a rejection filter — no case is rerolled — so shrinking stays
/// coherent.
pub(super) fn deps_for(slot: usize, picks: Option<&[usize]>) -> Vec<usize> {
    let mut deps: Vec<usize> = picks
        .into_iter()
        .flatten()
        .copied()
        .filter(|dep| *dep < slot)
        .collect();
    deps.sort_unstable();
    deps.dedup();
    deps
}

/// Guarantee one cross-target dependency when the domain admits one.
///
/// `OBL-E2E`'s non-vacuity list includes "cross-target dependencies present".
/// A single-target manifest has no lower slot to depend on, so the guarantee
/// applies only from two targets up; there the last target is given a
/// dependency on slot zero whenever no drawn target carried one.
pub(super) fn ensure_cross_target_dependency(targets: &mut [TargetPlan]) {
    if targets.len() < 2 || targets.iter().any(|target| !target.deps.is_empty()) {
        return;
    }
    if let Some(last) = targets.last_mut() {
        last.deps.push(0);
    }
}

/// Render a dependency list, empty when the target depends on nothing.
fn dependency_list(deps: &[usize]) -> StringOrList {
    if deps.is_empty() {
        StringOrList::Empty
    } else {
        StringOrList::List(deps.iter().copied().map(target_name).collect())
    }
}

/// The output path naming pool slot `slot`.
///
/// The generated names carry no whitespace or control characters, so the
/// emitted `build` and `default` lines need no Ninja escaping and a diagnostic
/// can quote them directly.
pub(crate) fn target_name(slot: usize) -> String {
    format!("out/{slot:04}")
}

/// Generate a bounded, well-formed [`ManifestSpec`].
///
/// Rules, targets, and defaults are drawn independently, so the shapes the
/// obligation names are reachable but not guaranteed on any one case; the
/// coverage tally in the declaration suite records the classes a run actually
/// reached rather than assuming them.
pub(crate) fn arb_manifest_spec() -> impl Strategy<Value = ManifestSpec> {
    (MIN_TARGETS..=MAX_TARGETS, MIN_RULES..=MAX_RULES)
        .prop_flat_map(|(target_count, rule_count)| {
            let slots: Vec<usize> = (0..target_count).collect();
            (
                Just(target_count),
                Just(rule_count),
                prop::collection::vec(0usize..rule_count, target_count),
                prop::collection::vec(
                    prop::collection::vec(0usize..MAX_TARGETS, 0..=MAX_DEPS_PER_TARGET),
                    target_count,
                ),
                prop::collection::vec(any::<bool>(), target_count),
                prop::collection::vec(0usize..MAX_TARGETS, 0..=MAX_DEFAULT_PICKS),
                Just(slots).prop_shuffle(),
                1usize..=MAX_TARGETS,
            )
        })
        .prop_map(
            |(
                target_count,
                rule_count,
                rule_picks,
                dep_picks,
                serial_flags,
                default_picks,
                shuffled,
                rotation,
            )| {
                Blueprint {
                    target_count,
                    rule_count,
                    rule_picks,
                    dep_picks,
                    serial_flags,
                    default_picks,
                    shuffled,
                    rotation,
                }
                .build()
            },
        )
}
