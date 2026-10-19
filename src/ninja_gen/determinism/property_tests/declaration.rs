//! `OBL-E2E`: permuting target and default declaration order does not change
//! the emitted bytes, for manifests with distinct rule names.
//!
//! This is the end-to-end arm of the determinism contract. `OBL-ORDER` states
//! the same shape over a `BuildGraph` built directly; this states it over a
//! *manifest*, so the lowering from `NetsukeManifest` to `BuildGraph` runs
//! inside the comparison. A defect in either half — `process_rules`,
//! `process_targets`, `process_defaults`, or the emitter's ordering rules —
//! shows up as a textual difference between two declarations of one manifest.
//!
//! Three decisions carry the obligation:
//!
//! - **The comparison is between two inputs, not an input and a normal form.**
//!   Both manifests are rebuilt from one [`ManifestSpec`] under two declaration
//!   orders, so any divergence is attributable to declaration order. An oracle
//!   computed here would only mirror the production call.
//! - **`defaults` is permuted with `targets`.** Lowering copies the declared
//!   list verbatim, and the emitter's only ordering of it is
//!   `ordered_default_targets`. A targets-only transform would leave
//!   `graph.default_targets` unchanged and `MUT-DEFSORT` could not be observed
//!   at all. The generator declares a descending default list, which also
//!   satisfies the obligation's "not already ascending" domain clause.
//! - **Rule names are pairwise distinct, and the excluded class is pinned.**
//!   `process_rules` is last-writer-wins, so two same-named rules make
//!   declaration order observable. The permitted domain avoids that shape by
//!   construction; [`same_named_rules_reorder_emission`] demonstrates with a
//!   directed witness that the exclusion is load-bearing rather than
//!   decorative.
//!
//! What this obligation cannot see: `from_manifest` never produces
//! `implicit_outputs`, `pool`, `depfile`, `deps_format`, or `restat`. Those
//! emitter branches are `OBL-ORDER`'s direct-graph domain, not this one. This
//! obligation is stronger over the lowering and weaker over the emitter's
//! input domain; neither subsumes the other.

use crate::ast::{NetsukeManifest, Recipe, Rule, StringOrList, Target, Vars};
use crate::ir::BuildGraph;
use crate::ninja_gen::RecipeShell;
use crate::ninja_gen::determinism::strategy::manifest::{ManifestSpec, arb_manifest_spec};
use crate::ninja_gen::generate_bundle;
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use std::cell::Cell;

use super::support::coverage_runner;

proptest! {
    // A case lowers and emits up to thirty targets and twelve rules, several
    // times, so the budget matches `order`'s end-to-end arm rather than the
    // cheaper rule-level suites.
    #![proptest_config(ProptestConfig { cases: 128, .. ProptestConfig::default() })]

    /// Lowering and emitting two declaration orders of one manifest agree.
    ///
    /// The two manifests hold identical rules, targets, and defaults; only the
    /// order of the `targets` and `defaults` lists differs. Any textual
    /// difference therefore names declaration order as the cause.
    #[test]
    fn declaration_order_does_not_change_emission(spec in arb_manifest_spec()) {
        let (first_targets, second_targets) = spec.target_orders();
        let (first_defaults, second_defaults) = spec.default_orders();
        let first = emit_manifest(&spec.to_manifest(first_targets, first_defaults))?;
        let second = emit_manifest(&spec.to_manifest(second_targets, second_defaults))?;

        prop_assert_eq!(
            &first,
            &second,
            "the emitted bundle must depend on the manifest's contents, not on \
             the order its targets and defaults are declared in; first order \
             {:?}, second order {:?}",
            first_targets,
            second_targets,
        );
    }

    /// Every declared default names a target the same manifest declares.
    ///
    /// A generator defect rather than a determinism law, but it belongs here:
    /// a default naming an undeclared target would be emitted identically
    /// under both orders, so the property above would pass while producing a
    /// file real Ninja rejects. Lowering does not validate `defaults`, so
    /// nothing else in the repository catches it.
    #[test]
    fn declared_defaults_name_declared_targets(spec in arb_manifest_spec()) {
        let (targets, _) = spec.target_orders();
        let manifest = spec.to_manifest(targets, spec.default_orders().0);
        let names: Vec<&str> = target_names(&manifest);

        for declared in &manifest.defaults {
            prop_assert!(
                names.contains(&declared.as_str()),
                "declared default {declared:?} is not one of the manifest's \
                 targets {names:?}",
            );
        }
    }
}

/// A manifest's complete emitted bundle, as one comparable string.
///
/// The sidecars are folded in beside the build file because `OBL-E2E` is
/// stated over the artefact rather than over one file: a sidecar whose path or
/// body depended on declaration order would otherwise pass unnoticed. Paths
/// are included because they are content-addressed, so two bundles differing
/// only in a sidecar name are still different artefacts.
///
/// # Errors
///
/// Returns a test failure when a generated manifest does not lower or the
/// emitter refuses the resulting graph. The generated domain is well formed by
/// construction, so either is a generator defect rather than an expected
/// outcome; reporting it as a shrinkable failure keeps the counter-example.
fn emit_manifest(manifest: &NetsukeManifest) -> Result<String, TestCaseError> {
    let graph = lower(manifest)?;
    let bundle = generate_bundle(&graph).map_err(|error| {
        TestCaseError::fail(format!("a well-formed generated graph must emit: {error}"))
    })?;
    let mut emitted = bundle.build_file().to_owned();
    for sidecar in bundle.dyndep_files() {
        emitted.push_str(sidecar.relative_path().as_str());
        emitted.push('\n');
        emitted.push_str(sidecar.content());
    }
    Ok(emitted)
}

/// Lower one manifest, reporting a generator defect as a test failure.
///
/// # Errors
///
/// Returns a test failure when the manifest does not lower.
fn lower(manifest: &NetsukeManifest) -> Result<BuildGraph, TestCaseError> {
    BuildGraph::from_manifest_for_shell(manifest, RecipeShell::Posix).map_err(|error| {
        TestCaseError::fail(format!(
            "a well-formed generated manifest must lower: {error}"
        ))
    })
}

/// A manifest's target names, in declaration order.
fn target_names(manifest: &NetsukeManifest) -> Vec<&str> {
    manifest
        .targets
        .iter()
        .filter_map(|target| match &target.name {
            StringOrList::String(name) => Some(name.as_str()),
            _ => None,
        })
        .collect()
}

/// The classes a run must reach for this obligation to mean anything.
///
/// Each field is a count of cases, never an assertion inside the property: the
/// generated domain admits every shape but does not guarantee any one of them on
/// a given draw. The tally turns "the run happened to contain only easy cases"
/// from an invisible possibility into a failure.
#[derive(Clone, Copy, Debug, Default)]
struct RunShape {
    /// Cases where the two target orders differed.
    reordered: usize,
    /// Cases where two targets share one interned action.
    interned: usize,
    /// Cases declaring at least one default.
    with_defaults: usize,
    /// Cases whose default declaration is not already ascending.
    descending_defaults: usize,
    /// Cases declaring a cross-target dependency.
    cross_target_deps: usize,
    /// Cases reaching all of the above at once.
    intersections: usize,
}

impl RunShape {
    /// Fold one specification's shape into this tally.
    fn extended(self, spec: &ManifestSpec) -> Self {
        let (first_targets, second_targets) = spec.target_orders();
        let (declared, _) = spec.default_orders();
        let manifest = spec.to_manifest(first_targets, declared);
        let target_count = manifest.targets.len();
        let action_count = lowered_action_count(&manifest);
        let reordered = first_targets != second_targets;
        let interned = action_count < target_count;
        let with_defaults = !declared.is_empty();
        // A slice pattern rather than `pair[0] > pair[1]`: the pair is known to
        // be two elements long, but the workspace denies unchecked indexing
        // outright, so the destructuring doubles as the length proof.
        let descending_defaults = declared.len() >= 2
            && declared
                .windows(2)
                .all(|pair| matches!(pair, [higher, lower] if higher > lower));
        let cross_target_deps = has_cross_target_dependency(&manifest);
        Self {
            reordered: self.reordered + usize::from(reordered),
            interned: self.interned + usize::from(interned),
            with_defaults: self.with_defaults + usize::from(with_defaults),
            descending_defaults: self.descending_defaults + usize::from(descending_defaults),
            cross_target_deps: self.cross_target_deps + usize::from(cross_target_deps),
            intersections: self.intersections
                + usize::from(reordered && interned && with_defaults && cross_target_deps),
        }
    }
}

/// How many distinct actions the manifest's lowering registers.
///
/// Read off a real lowering rather than predicted from the specification: the
/// interning decision belongs to `register_action`, and reproducing it here
/// would make the tally agree with a copy of the production logic instead of
/// with production. A manifest that failed to lower counts as zero, which can
/// only over-report interning; the property above reports the failure itself.
fn lowered_action_count(manifest: &NetsukeManifest) -> usize {
    lower(manifest).map_or(0, |graph| graph.actions.len())
}

/// Whether any target depends on an output another target declares.
fn has_cross_target_dependency(manifest: &NetsukeManifest) -> bool {
    let names = target_names(manifest);
    manifest.targets.iter().any(|target| match &target.deps {
        StringOrList::List(deps) => deps.iter().any(|dep| names.contains(&dep.as_str())),
        _ => false,
    })
}

/// The generated domain reaches every class `OBL-E2E` names.
///
/// A run whose permutations never reordered, whose manifests never interned two
/// targets, or which never declared defaults would satisfy the property above
/// while exercising none of the machinery it exists to protect. The tally lives
/// in a `Cell` because `TestRunner::run` takes an `Fn`.
#[test]
fn the_run_reaches_every_required_class() {
    let tally: Cell<RunShape> = Cell::new(RunShape::default());
    coverage_runner(128)
        .run(&arb_manifest_spec(), |spec| {
            tally.set(tally.get().extended(&spec));
            Ok(())
        })
        .expect("the manifest strategy should be generatable");

    let observed = tally.into_inner();
    macro_rules! covered {
        ($field:ident, $why:expr) => {
            assert!(observed.$field > 0, "{}, reached: {observed:?}", $why);
        };
    }
    covered!(
        reordered,
        "no case's two declaration orders differed, so the property compared one \
         arrangement to itself and is inconclusive rather than passed"
    );
    covered!(
        interned,
        "no case had two targets collapse onto one interned action, so the \
         interning path never reached the comparison"
    );
    covered!(
        with_defaults,
        "no case declared a default target, so the `default` line was never emitted"
    );
    covered!(
        descending_defaults,
        "no case declared two or more distinct defaults in descending order, so \
         `MUT-DEFSORT` would be unobservable even if applied"
    );
    covered!(
        cross_target_deps,
        "no case declared a cross-target dependency, so the dependency-bearing \
         `build` lines never reached the comparison"
    );
    covered!(
        intersections,
        "no case reached reordering, interning, defaults, and cross-target \
         dependencies together, which is the intersection OBL-E2E names"
    );
}

/// Two same-named rules with different bodies make declaration order observable.
///
/// The directed witness for the obligation's excluded class. `process_rules` is
/// last-writer-wins and no duplicate-rule error exists, so permuting two rules
/// named alike changes which body every referencing target resolves — and
/// therefore changes the emission. The property's distinct-names precondition
/// exists precisely to exclude this shape; without the witness, the
/// precondition would be an unexamined escape hatch.
///
/// The two manifests differ only in the order of the two rule declarations.
/// Their emitted text must *differ*; if it did not, either `process_rules` had
/// stopped being last-writer-wins or the rules had stopped being distinct in
/// body, and the precondition would be describing a boundary that no longer
/// exists.
#[test]
fn same_named_rules_reorder_emission() {
    let first = emit_manifest(&duplicate_rule_manifest([
        "echo first-body",
        "echo second-body",
    ]))
    .expect("the duplicate-rule manifest has distinct target outputs and must emit");
    let second = emit_manifest(&duplicate_rule_manifest([
        "echo second-body",
        "echo first-body",
    ]))
    .expect("the reordered duplicate-rule manifest must emit");

    assert_ne!(
        first, second,
        "two same-named rules with different bodies must make declaration \
         order observable; if these agree, the obligation's distinct-names \
         precondition no longer describes an exclusion that exists. Emitted:\n{first}",
    );
}

/// A manifest declaring two rules named alike, each carrying one body.
fn duplicate_rule_manifest(bodies: [&str; 2]) -> NetsukeManifest {
    NetsukeManifest {
        netsuke_version: semver::Version::new(1, 0, 0),
        vars: Vars::default(),
        macros: Vec::new(),
        rules: bodies.into_iter().map(duplicate_rule).collect(),
        actions: Vec::new(),
        targets: vec![rule_backed_target("out/one"), rule_backed_target("out/two")],
        defaults: Vec::new(),
    }
}

/// A rule named `dup` whose command is `body`.
fn duplicate_rule(body: &str) -> Rule {
    Rule {
        name: "dup".to_owned(),
        recipe: Recipe::Command {
            command: StringOrList::String(body.to_owned()),
        },
        description: None,
    }
}

/// A target whose output is `name` and whose recipe is the `dup` rule.
fn rule_backed_target(name: &str) -> Target {
    Target {
        name: StringOrList::String(name.to_owned()),
        recipe: Recipe::Rule {
            rule: StringOrList::String("dup".to_owned()),
        },
        sources: StringOrList::Empty,
        deps: StringOrList::Empty,
        dependency_order: crate::ast::DependencyOrder::Parallel,
        order_only_deps: StringOrList::Empty,
        vars: Vars::default(),
        phony: false,
        always: false,
        conditional: false,
        description: None,
    }
}
