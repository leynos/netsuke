//! The generated-manifest model and the strategy that produces it.
//!
//! Split from `fidelity_property/mod.rs`, which holds the property itself, to
//! keep both files under Whitaker's raw-line module cap. A generated manifest is
//! held as data — [`Spec`] over [`Target`] over [`Construct`] — so its YAML, its
//! coverage, and the strategy's combinations are all derived from one value
//! rather than maintained as parallel descriptions.
//!
//! The model is crate-private: `fidelity_property` re-exports the pieces its
//! run needs through `pub(super)`, and nothing outside that module tree names
//! these types.

use std::cell::Cell;
use std::fmt::Write as _;

use proptest::{prelude::*, sample::select};

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
pub(super) struct Spec {
    /// Targets, in manifest order.
    targets: Vec<Target>,
    /// Whether a top-level Boolean variable is declared.
    has_vars: bool,
    /// Whether a macro is declared.
    has_macro: bool,
}

/// The number of times each construct appears in one spec.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Coverage {
    /// `Boolean` occurrences.
    pub(super) boolean: usize,
    /// Lexical path filter occurrences.
    pub(super) path_filters: usize,
    /// Collection filter occurrences.
    pub(super) collection_filters: usize,
    /// `timedelta` occurrences.
    pub(super) timedelta: usize,
    /// `now()` occurrences.
    pub(super) now: usize,
    /// Macro-call occurrences.
    pub(super) macro_calls: usize,
    /// `foreach` target occurrences.
    pub(super) foreach_targets: usize,
    /// `when` clause occurrences.
    pub(super) whens: usize,
    /// Target occurrences.
    pub(super) targets: usize,
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
pub(super) struct Tally {
    /// Construct counts accumulated so far.
    coverage: Cell<Coverage>,
}

impl Tally {
    /// Record one generated spec.
    pub(super) fn record(&self, coverage: Coverage) {
        let mut total = self.coverage.get();
        total.absorb(coverage);
        self.coverage.set(total);
    }

    /// Return the tally recorded so far.
    pub(super) fn get(&self) -> Coverage {
        self.coverage.get()
    }
}

impl Spec {
    /// Render the manifest as YAML.
    pub(super) fn yaml(&self) -> String {
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
    pub(super) fn coverage(&self) -> Coverage {
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
pub(super) fn spec_strategy() -> impl Strategy<Value = Spec> {
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
