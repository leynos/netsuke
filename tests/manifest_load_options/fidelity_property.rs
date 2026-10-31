//! Differential property: a pure manifest means the same thing in both modes.
//!
//! The claim is narrow on purpose. It covers manifests built only from
//! constructs the test surface already admits — variables, macros, `foreach`
//! over literal lists, `when` comparisons, Boolean interpolation, the lexical
//! path filters, the collection filters, `timedelta`, and `now()` under an
//! injected clock — and says that the build surface and the test surface
//! produce the same manifest, the same graph, and the same Ninja text for
//! them. A manifest that reaches the host is covered by the corpus run, which
//! expectation is "equal *or* a typed refusal"; here every generated manifest
//! must load in both modes.
//!
//! Three comparison layers are recorded rather than one because each can drift
//! independently. The manifest catches a rendering difference; the [`GraphView`]
//! projection catches a difference in how the rendered manifest was interpreted;
//! and the Ninja text catches a difference in the final emitted build file.
//! Comparing only the manifest would call two loads equal whose recipes were
//! lowered differently.
//!
//! The generator is valid by construction rather than filtered after the fact:
//! a rejected case would silently shrink the domain and the property would
//! weaken without saying so. Two counters keep the coverage honest — every
//! construct must be generated at least ten times in a run, so a strategy that
//! stops producing, say, `timedelta` fails loudly instead of passing over a
//! smaller grammar.

use std::{cell::Cell, fmt::Write as _};

use anyhow::{Context as _, Result};
use netsuke::{
    ast::NetsukeManifest,
    graph_view::GraphView,
    ir::BuildGraph,
    manifest::{
        EnvAccessPolicy, ManifestEnvironment, ManifestLoadOptions, ManifestName, TestSurface,
        from_str_with_options,
    },
    ninja_gen::generate_with_shell,
    recipe_shell::RecipeShell,
};
use proptest::{
    prelude::*,
    sample::select,
    test_runner::{Config as ProptestConfig, TestCaseError, TestRunner},
};
use rstest::rstest;
use time::OffsetDateTime;
use time::macros::datetime;

use super::fixtures::{fixed_clock_config, one_var_reader};

/// The instant both surfaces are configured with, so `now()` agrees.
const FIXED_INSTANT: OffsetDateTime = datetime!(2026-06-08 12:00:00 UTC);

/// How many generated manifests a run must contain.
const CASES: u32 = 128;

/// How many times each construct must be generated in a run.
///
/// Ten is low enough that a 128-case run clears it comfortably and high enough
/// that a construct which has dropped out of the strategy cannot pass by luck.
const MIN_CONSTRUCT_COVERAGE: usize = 10;

/// One template construct the property varies.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Construct {
    /// Boolean interpolation, which exercises the legacy formatter.
    Boolean,
    /// `basename`, one of the lexical path filters.
    Basename,
    /// `dirname`, one of the lexical path filters.
    Dirname,
    /// `with_suffix`, one of the lexical path filters.
    WithSuffix,
    /// `relative_to`, one of the lexical path filters.
    RelativeTo,
    /// `uniq`, one of the collection filters.
    Uniq,
    /// `flatten`, one of the collection filters.
    Flatten,
    /// `compact`, one of the collection filters.
    Compact,
    /// `group_by`, one of the collection filters.
    GroupBy,
    /// `timedelta`, a deterministic time function.
    TimeDelta,
    /// `now()`, deterministic only because a clock was injected.
    Now,
    /// A macro call defined in the same manifest.
    Macro,
}

/// Every construct the strategy may choose from.
const CONSTRUCTS: &[Construct] = &[
    Construct::Boolean,
    Construct::Basename,
    Construct::Dirname,
    Construct::WithSuffix,
    Construct::RelativeTo,
    Construct::Uniq,
    Construct::Flatten,
    Construct::Compact,
    Construct::GroupBy,
    Construct::TimeDelta,
    Construct::Now,
    Construct::Macro,
];

impl Construct {
    /// Return the template expression that exercises this construct.
    fn expression(self) -> &'static str {
        match self {
            Self::Boolean => "{{ flag }}",
            Self::Basename => "{{ 'dir/file.tar.gz' | basename }}",
            Self::Dirname => "{{ 'dir/file.tar.gz' | dirname }}",
            Self::WithSuffix => "{{ 'file.txt' | with_suffix('.md') }}",
            Self::RelativeTo => "{{ 'dir/file.txt' | relative_to('dir') }}",
            Self::Uniq => "{{ ['b', 'a', 'a'] | uniq | join(',') }}",
            Self::Flatten => "{{ [['a'], ['b']] | flatten | join(',') }}",
            Self::Compact => "{{ ['a', '', none] | compact | join(',') }}",
            Self::GroupBy => {
                "{{ ([{'kind': 'tool'}, {'kind': 'tool'}] | group_by('kind')).tool | length }}"
            }
            Self::TimeDelta => "{{ timedelta(seconds=5) }}",
            Self::Now => "{{ now() }}",
            Self::Macro => "{{ shout('hello') }}",
        }
    }
}

/// One target in a generated manifest.
#[derive(Clone, Debug)]
struct Target {
    /// Position in the manifest, used to keep names unique.
    index: usize,
    /// Whether the target iterates a literal three-item list.
    is_foreach: bool,
    /// Constructs this target's command interpolates.
    constructs: Vec<Construct>,
    /// Whether the target declares a source.
    has_source: bool,
}

/// A generated manifest, held as data so its YAML and its coverage are both
/// derived from the same value.
#[derive(Clone, Debug)]
struct Spec {
    /// Targets, in manifest order.
    targets: Vec<Target>,
    /// Whether a top-level Boolean variable is declared.
    has_vars: bool,
    /// Whether a macro is declared.
    has_macro: bool,
}

/// The number of times each construct appears in one spec.
#[derive(Clone, Copy, Debug, Default)]
struct Coverage {
    /// `Boolean` occurrences.
    boolean: usize,
    /// Lexical path filter occurrences.
    path_filters: usize,
    /// Collection filter occurrences.
    collection_filters: usize,
    /// `timedelta` occurrences.
    timedelta: usize,
    /// `now()` occurrences.
    now: usize,
    /// Macro-call occurrences.
    macro_calls: usize,
    /// `foreach` target occurrences.
    foreach_targets: usize,
    /// `when` clause occurrences.
    whens: usize,
    /// Target occurrences.
    targets: usize,
}

impl Coverage {
    /// Add `other` into this tally.
    fn absorb(&mut self, other: Self) {
        self.boolean += other.boolean;
        self.path_filters += other.path_filters;
        self.collection_filters += other.collection_filters;
        self.timedelta += other.timedelta;
        self.now += other.now;
        self.macro_calls += other.macro_calls;
        self.foreach_targets += other.foreach_targets;
        self.whens += other.whens;
        self.targets += other.targets;
    }
}

/// The running tally for one property run.
#[derive(Debug, Default)]
struct Tally {
    /// Construct counts accumulated so far.
    coverage: Cell<Coverage>,
}

impl Tally {
    /// Record one generated spec.
    fn record(&self, coverage: Coverage) {
        let mut total = self.coverage.get();
        total.absorb(coverage);
        self.coverage.set(total);
    }

    /// Return the tally recorded so far.
    fn get(&self) -> Coverage {
        self.coverage.get()
    }
}

impl Spec {
    /// Render the manifest as YAML.
    fn yaml(&self) -> String {
        let mut yaml = String::from("netsuke_version: \"1.0.0\"\n");
        if self.has_vars {
            yaml.push_str("vars:\n  flag: true\n");
        }
        if self.has_macro {
            yaml.push_str(
                "macros:\n  - signature: \"shout(text)\"\n    body: \"{{ text | upper }}\"\n",
            );
        }
        yaml.push_str("targets:\n");
        for target in &self.targets {
            Self::write_target(&mut yaml, target);
        }
        yaml
    }

    /// Append one target's YAML to `yaml`.
    fn write_target(yaml: &mut String, target: &Target) {
        let name = if target.is_foreach {
            format!("out{}-{{{{ item }}}}.txt", target.index)
        } else {
            format!("out{}.txt", target.index)
        };
        let command = target
            .constructs
            .iter()
            .map(|construct| construct.expression())
            .collect::<Vec<_>>()
            .join(" ");
        writeln!(yaml, "  - name: \"{name}\"").expect("writing to a String cannot fail");
        writeln!(yaml, "    command: \"echo {command}\"").expect("writing to a String cannot fail");
        if target.has_source {
            writeln!(yaml, "    sources: \"in{}.txt\"", target.index)
                .expect("writing to a String cannot fail");
        }
        if target.is_foreach {
            yaml.push_str("    foreach:\n      - alpha\n      - beta\n      - gamma\n");
            writeln!(yaml, "    when: \"item != 'skip'\"")
                .expect("writing to a String cannot fail");
        } else if target.index == 0 {
            // A comparison, not a Boolean variable: `when` must accept an
            // expression and the two modes must agree on its result.
            writeln!(yaml, "    when: \"1 < 2\"").expect("writing to a String cannot fail");
        }
    }

    /// Count what this spec exercises.
    fn coverage(&self) -> Coverage {
        let mut coverage = Coverage {
            targets: self.targets.len(),
            ..Coverage::default()
        };
        for target in &self.targets {
            coverage.foreach_targets += usize::from(target.is_foreach);
            coverage.whens += usize::from(target.is_foreach || target.index == 0);
            for construct in &target.constructs {
                match construct {
                    Construct::Boolean => coverage.boolean += 1,
                    Construct::Basename
                    | Construct::Dirname
                    | Construct::WithSuffix
                    | Construct::RelativeTo => coverage.path_filters += 1,
                    Construct::Uniq
                    | Construct::Flatten
                    | Construct::Compact
                    | Construct::GroupBy => coverage.collection_filters += 1,
                    Construct::TimeDelta => coverage.timedelta += 1,
                    Construct::Now => coverage.now += 1,
                    Construct::Macro => coverage.macro_calls += 1,
                }
            }
        }
        coverage
    }
}

/// Build a spec from the strategy's raw choices.
///
/// The first target always carries the full deterministic repertoire, so every
/// construct is exercised at least once per case; the per-target extras then
/// vary the combinations. Constructs whose precondition is absent — a Boolean
/// variable or a macro — are dropped here rather than left to fail a load.
fn build_spec(extras: Vec<Vec<Construct>>, has_vars: bool, has_macro: bool, mask: u8) -> Spec {
    let targets = extras
        .into_iter()
        .enumerate()
        .map(|(index, mut constructs)| {
            if index == 0 {
                let mut repertoire = vec![
                    Construct::Basename,
                    Construct::Dirname,
                    Construct::WithSuffix,
                    Construct::RelativeTo,
                    Construct::Uniq,
                    Construct::Flatten,
                    Construct::Compact,
                    Construct::GroupBy,
                    Construct::TimeDelta,
                    Construct::Now,
                ];
                if has_vars {
                    repertoire.push(Construct::Boolean);
                }
                if has_macro {
                    repertoire.push(Construct::Macro);
                }
                repertoire.append(&mut constructs);
                constructs = repertoire;
            }
            constructs.retain(|construct| match construct {
                Construct::Boolean => has_vars,
                Construct::Macro => has_macro,
                _ => true,
            });
            Target {
                index,
                is_foreach: (mask >> index) & 1 == 1,
                constructs,
                has_source: index.is_multiple_of(2),
            }
        })
        .collect();
    Spec {
        targets,
        has_vars,
        has_macro,
    }
}

/// The strategy the property runs over.
fn spec_strategy() -> impl Strategy<Value = Spec> {
    (
        prop::collection::vec(prop::collection::vec(select(CONSTRUCTS), 0..=2), 1..=4),
        any::<bool>(),
        any::<bool>(),
        any::<u8>(),
    )
        .prop_map(|(extras, has_vars, has_macro, mask)| {
            build_spec(extras, has_vars, has_macro, mask)
        })
}

/// Load `yaml` through the build registration with a fixed clock.
fn load_full(yaml: &str, config: &netsuke::stdlib::StdlibConfig) -> Result<NetsukeManifest> {
    let reader = one_var_reader();
    let environment = ManifestEnvironment::new(&reader, EnvAccessPolicy::default());
    from_str_with_options(
        yaml,
        &ManifestName::new("Netsukefile"),
        ManifestLoadOptions::full(config.clone(), environment),
    )
}

/// Load `yaml` through the test registration with the same fixed clock.
fn load_test(yaml: &str, config: &netsuke::stdlib::StdlibConfig) -> Result<NetsukeManifest> {
    let surface = TestSurface::new(config.clone()).with_env_reader(one_var_reader());
    from_str_with_options(
        yaml,
        &ManifestName::new("Netsukefile"),
        ManifestLoadOptions::test(surface),
    )
}

/// Fail a property case with a message naming the spec that produced it.
fn fail(spec: &Spec, message: impl std::fmt::Display) -> TestCaseError {
    TestCaseError::fail(format!("{message}\n--- manifest ---\n{}", spec.yaml()))
}

/// Compare the manifest, graph projection, and Ninja text of one spec.
fn compare_modes(
    spec: &Spec,
    config: &netsuke::stdlib::StdlibConfig,
) -> std::result::Result<(), TestCaseError> {
    let yaml = spec.yaml();
    let full = load_full(&yaml, config)
        .map_err(|error| fail(spec, format_args!("Full load failed: {error:#}")))?;
    let test = load_test(&yaml, config)
        .map_err(|error| fail(spec, format_args!("Test load failed: {error:#}")))?;

    let full_json = serde_json_canonicalizer::to_string(&full).map_err(|error| {
        fail(
            spec,
            format_args!("Full manifest did not serialize: {error}"),
        )
    })?;
    let test_json = serde_json_canonicalizer::to_string(&test).map_err(|error| {
        fail(
            spec,
            format_args!("Test manifest did not serialize: {error}"),
        )
    })?;
    if full_json != test_json {
        return Err(fail(
            spec,
            format_args!("manifests differ:\nFull: {full_json}\nTest: {test_json}"),
        ));
    }

    let full_graph = BuildGraph::from_manifest_for_shell(&full, RecipeShell::Posix)
        .map_err(|error| fail(spec, format_args!("Full graph failed: {error}")))?;
    let test_graph = BuildGraph::from_manifest_for_shell(&test, RecipeShell::Posix)
        .map_err(|error| fail(spec, format_args!("Test graph failed: {error}")))?;
    if GraphView::from_build_graph(&full_graph) != GraphView::from_build_graph(&test_graph) {
        return Err(fail(spec, "graph projections differ"));
    }

    let full_ninja = generate_with_shell(&full_graph, RecipeShell::Posix)
        .map_err(|error| fail(spec, format_args!("Full Ninja generation failed: {error}")))?;
    let test_ninja = generate_with_shell(&test_graph, RecipeShell::Posix)
        .map_err(|error| fail(spec, format_args!("Test Ninja generation failed: {error}")))?;
    if full_ninja != test_ninja {
        return Err(fail(
            spec,
            format_args!("Ninja text differs:\nFull:\n{full_ninja}\nTest:\n{test_ninja}"),
        ));
    }
    Ok(())
}

/// Every construct clears the per-run coverage floor.
fn assert_coverage(coverage: Coverage) {
    let required = [
        ("targets", coverage.targets),
        ("foreach targets", coverage.foreach_targets),
        ("when clauses", coverage.whens),
        ("Boolean interpolation", coverage.boolean),
        ("lexical path filters", coverage.path_filters),
        ("collection filters", coverage.collection_filters),
        ("timedelta", coverage.timedelta),
        ("now()", coverage.now),
        ("macro calls", coverage.macro_calls),
    ];
    for (name, count) in required {
        assert!(
            count >= MIN_CONSTRUCT_COVERAGE,
            "the strategy generated `{name}` {count} times, fewer than {MIN_CONSTRUCT_COVERAGE}; \
             the grammar is narrower than the property claims"
        );
    }
}

/// The build and test surfaces agree on every generated pure manifest.
#[rstest]
fn full_and_test_agree_on_generated_manifests() -> Result<()> {
    let config = fixed_clock_config()?.with_clock(netsuke::stdlib::fixed_clock(FIXED_INSTANT));
    let tally = Tally::default();
    let mut runner = TestRunner::new(ProptestConfig {
        cases: CASES,
        ..ProptestConfig::default()
    });
    runner
        .run(&spec_strategy(), |spec| {
            tally.record(spec.coverage());
            compare_modes(&spec, &config)
        })
        .context("the Full/Test differential property failed")?;
    assert_coverage(tally.get());
    Ok(())
}
