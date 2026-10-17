//! `OBL-DEFAULT`: the `default` line is the ascending sort of the graph's
//! `default_targets`, and it follows every `build` statement.
//!
//! Two claims that fail for different reasons. The ordering half is a plain
//! sort whose oracle is a sorted clone computed here — weak alone, because it
//! mirrors the production call — which is why the permutation arm beside it
//! compares two *inputs* instead and so cannot be satisfied by copied code.
//! The positional half is correctness rather than style: real Ninja rejects a
//! `default` statement naming a target whose `build` statement it has not yet
//! seen, so moving the block above edge rendering breaks the file.
//!
//! The emitted operands are compared as a **multiset**. `Vec::sort` does not
//! deduplicate and a graph may legitimately name one target twice, so the
//! duplicate-bearing witness near the foot of the module pins the behaviour a
//! future switch to a set would otherwise silently change.
//!
//! The entry point is [`generate_bundle`] rather than
//! [`crate::ninja_gen::generate`], for the reason `OBL-NOLOSS` records: it is
//! the entry point both production paths agree on, so the positional assertion
//! covers `dyndep`'s block as well as the single-file renderer's.
//!
//! The generator and the graph fixtures it composes live in [`generator`], so
//! this module reads as statements about emitted text.

mod generator;

use crate::ir::BuildGraph;
use crate::ninja_gen::generate_bundle;
use camino::Utf8PathBuf;
use generator::{action, arb_default_case, edge};
use proptest::prelude::*;
use proptest::test_runner::{TestCaseError, TestRunner};
use std::cell::Cell;

proptest! {
    // Emitting one of twenty single-output edges is string building over a
    // handful of `default` operands, so the key suite's budget is affordable.
    #![proptest_config(ProptestConfig { cases: 128, .. ProptestConfig::default() })]

    /// The `default` line's operands are the declared targets, sorted ascending.
    ///
    /// Duplicates are preserved: a graph naming one target twice emits two
    /// operands, because the declaration is a multiset and `sort` does not
    /// deduplicate it. The empty case is asserted separately rather than folded
    /// into an equality, since "no defaults" is a shape with no line at all.
    #[test]
    fn the_default_line_sorts_its_operands(case in arb_default_case()) {
        let text = emit(&case.graph)?;
        let lines = default_lines(&text);
        if case.declared.is_empty() {
            prop_assert!(
                lines.is_empty(),
                "a graph declaring no defaults must emit no `default` line; \
                 emitted text:\n{text}",
            );
            return Ok(());
        }
        // A slice pattern rather than `lines[0]`: the binding both asserts the
        // exactly-one shape and yields the line, and the workspace denies
        // `clippy::indexing_slicing` besides.
        let [line] = lines.as_slice() else {
            return Err(TestCaseError::fail(format!(
                "a graph declaring defaults must emit exactly one `default` \
                 line, found {}; emitted text:\n{text}",
                lines.len(),
            )));
        };
        let mut expected: Vec<&str> = case.declared.iter().map(|path| path.as_str()).collect();
        expected.sort_unstable();
        prop_assert_eq!(
            line.operands.clone(),
            expected,
            "the `default` operands must be the declared targets in ascending \
             order, duplicates included; emitted text:\n{}",
            text,
        );
    }

    /// The `default` line comes after every `build` line.
    ///
    /// The offsets are byte offsets into the emitted text rather than line
    /// indices, because that is the form `OBL-DEFAULT` states and because a
    /// line-index comparison would silently pass if a `build` line were ever
    /// embedded mid-line in some other statement.
    #[test]
    fn the_default_line_follows_every_build_line(case in arb_default_case()) {
        let text = emit(&case.graph)?;
        let lines = default_lines(&text);
        if case.declared.is_empty() {
            prop_assert!(lines.is_empty());
            return Ok(());
        }
        let Some(line) = lines.first() else {
            return Err(TestCaseError::fail(format!(
                "a graph declaring {} defaults emitted no `default` line",
                case.declared.len(),
            )));
        };
        let default_offset = line.offset;
        let last_build = last_build_offset(&text);
        prop_assert!(
            last_build.is_some_and(|build| build < default_offset),
            "every `build` line must precede the `default` line, which sits at \
             byte {default_offset}; last build line at {last_build:?}. A \
             `default` naming a target Ninja has not yet seen is rejected. \
             Emitted text:\n{text}",
        );
    }

    /// Permuting the declared defaults does not change a byte of the text.
    ///
    /// This is the half the sorted-clone oracle cannot supply: it compares two
    /// *inputs* through the emitter rather than one input through a copy of the
    /// production sort. Reversing is used to perturb rather than a sampled
    /// shuffle because it is constructible and cheap; a palindromic multiset
    /// reverses to itself and then proves nothing on that case, which the
    /// run-shape tally measures rather than this property assuming.
    #[test]
    fn permuting_the_declared_defaults_does_not_change_the_text(case in arb_default_case()) {
        let mut permuted = case.graph.clone();
        permuted.default_targets.reverse();
        prop_assert_eq!(
            emit(&case.graph)?,
            emit(&permuted)?,
            "the emitted text must depend on the default target multiset, not \
             on the order it was declared in",
        );
    }
}

/// One emitted `default` line, with its byte offset and its operands.
struct DefaultLine<'a> {
    /// Byte offset of the line's first character in the emitted text.
    offset: usize,
    /// The line's operands, space-separated and unescaped.
    operands: Vec<&'a str>,
}

/// Every `default` line in `text`, in emission order.
fn default_lines(text: &str) -> Vec<DefaultLine<'_>> {
    let mut offset = 0;
    let mut found = Vec::new();
    for line in text.split_inclusive('\n') {
        if let Some(operands) = line.strip_prefix("default ") {
            found.push(DefaultLine {
                offset,
                operands: operands.split_whitespace().collect(),
            });
        }
        offset += line.len();
    }
    found
}

/// The byte offset of the last `build` line, if the text carries one.
fn last_build_offset(text: &str) -> Option<usize> {
    let mut offset = 0;
    let mut last = None;
    for line in text.split_inclusive('\n') {
        if line.starts_with("build ") {
            last = Some(offset);
        }
        offset += line.len();
    }
    last
}

/// Emit one graph, reporting a rejection as a shrinkable test failure.
///
/// Reported rather than unwrapped so a generator defect arrives as a failure
/// carrying the case, instead of as a panic shrinking cannot reduce. Every path
/// here is outside the reserved namespaces and free of unsupported characters,
/// so a refusal is a generator defect rather than an expected outcome.
///
/// # Errors
///
/// Returns a test failure when the emitter refuses the graph.
fn emit(graph: &BuildGraph) -> Result<String, TestCaseError> {
    generate_bundle(graph)
        .map(|bundle| bundle.build_file().to_owned())
        .map_err(|error| {
            TestCaseError::fail(format!("a well-formed generated graph must emit: {error}"))
        })
}

/// What a run's declared defaults looked like, tallied so it cannot go quiet.
///
/// The same shape language `OBL-PATHKEY` uses for its path lists, applied to the
/// default multiset. The two order-sensitive classes require two or more
/// *distinct* targets: a list whose entries are all equal is trivially both
/// ascending and descending and could never be reordered, so counting it would
/// report both extremes as exercised when neither had seen anything the other
/// had not.
#[derive(Clone, Copy, Debug, Default)]
struct DefaultShapes {
    /// Cases declaring no defaults at all.
    empty: usize,
    /// Cases declaring exactly one default.
    singleton: usize,
    /// Cases of two or more distinct targets already ascending.
    already_sorted: usize,
    /// Cases of two or more distinct targets in exact descending order.
    reverse_sorted: usize,
    /// Cases naming one target more than once.
    with_repeats: usize,
}

impl DefaultShapes {
    /// Fold one case's declared defaults into the tally.
    fn extended(self, declared: &[Utf8PathBuf]) -> Self {
        let shape = DeclaredShape::of(declared);
        Self {
            empty: self.empty + usize::from(declared.is_empty()),
            singleton: self.singleton + usize::from(declared.len() == 1),
            already_sorted: self.already_sorted + usize::from(shape.is_ascending),
            reverse_sorted: self.reverse_sorted + usize::from(shape.is_reversed),
            with_repeats: self.with_repeats + usize::from(shape.has_repeats),
        }
    }
}

/// One declaration's order-sensitive shape, relative to its sorted form.
///
/// Computed once so the tally reads as five questions rather than as five
/// expressions over the same three clones. `is_ascending` deliberately means
/// "two or more distinct entries already ascending": the class exists to show
/// the sort doing nothing, and a repeated single target would satisfy an
/// equality with the sorted form while showing nothing.
struct DeclaredShape {
    /// The declaration holds at least two distinct targets, ascending.
    is_ascending: bool,
    /// The declaration holds at least two distinct targets, descending.
    is_reversed: bool,
    /// Some target is named more than once.
    has_repeats: bool,
}

impl DeclaredShape {
    /// Classify one declaration.
    fn of(declared: &[Utf8PathBuf]) -> Self {
        let as_declared: Vec<&str> = declared.iter().map(|path| path.as_str()).collect();
        let mut ascending = as_declared.clone();
        ascending.sort_unstable();
        let mut distinct = ascending.clone();
        distinct.dedup();
        let is_distinct_multi = declared.len() >= 2 && declared.len() == distinct.len();
        let mut descending = ascending.clone();
        descending.reverse();
        Self {
            is_ascending: is_distinct_multi && as_declared == ascending,
            is_reversed: is_distinct_multi && as_declared == descending,
            has_repeats: declared.len() != distinct.len(),
        }
    }
}

/// The generated domain reaches every default shape the obligation names.
///
/// A run that never declared a default, or never declared two, would satisfy
/// every property above while leaving the sort and the positional rule
/// untested — the failure mode the plan's non-vacuity section exists to make
/// impossible. The tally lives in a `Cell` because `TestRunner::run` takes an
/// `Fn`.
#[test]
fn the_run_reaches_every_default_shape() {
    let tally: Cell<DefaultShapes> = Cell::new(DefaultShapes::default());
    TestRunner::new(ProptestConfig {
        cases: 128,
        ..ProptestConfig::default()
    })
    .run(&arb_default_case(), |case| {
        tally.set(tally.get().extended(&case.declared));
        Ok(())
    })
    .expect("the default strategy should be generatable");

    let observed = tally.into_inner();
    macro_rules! covered {
        ($field:ident, $why:expr) => {
            assert!(observed.$field > 0, "{}, reached: {observed:?}", $why);
        };
    }
    covered!(
        empty,
        "no case declared zero defaults, so the arm asserting no `default` line \
         is emitted was never exercised"
    );
    covered!(singleton, "no case declared exactly one default");
    covered!(
        already_sorted,
        "no case declared two or more distinct defaults already in ascending \
         order, so the arm where the sort is the identity was never exercised"
    );
    covered!(
        reverse_sorted,
        "no case declared two or more distinct defaults in descending order, so \
         the worst case for the sort was never exercised"
    );
    covered!(
        with_repeats,
        "no case named a target twice, so the decision that the sort does not \
         deduplicate was never exercised"
    );
}

/// A descending declaration is emitted ascending.
///
/// The directed witness for the sort. It is constructed rather than sampled so
/// the perturbation is guaranteed: `MUT-DEFSORT` removes the `.sorted()` call
/// inside `ordered_default_targets`, and this case then emits the declaration
/// as given and fails.
#[test]
fn a_descending_declaration_is_emitted_ascending() {
    let graph = graph_declaring(&["out/0002", "out/0001", "out/0000"]);
    let text = emit(&graph).expect("a well-formed graph must emit");
    assert_eq!(
        default_lines(&text)
            .first()
            .map(|line| line.operands.clone()),
        Some(vec!["out/0000", "out/0001", "out/0002"]),
        "the declaration is descending, so an emitted ascending order can only \
         come from the sort. Emitted text:\n{text}",
    );
}

/// A repeated target is emitted twice, not collapsed.
///
/// `Vec::sort` orders without deduplicating, which is the documented decision
/// this pins. Replacing the sort with a `BTreeSet` round trip would emit one
/// operand here, and no property over a duplicate-free domain would notice.
#[test]
fn a_repeated_target_is_emitted_twice() {
    let graph = graph_declaring(&["out/0001", "out/0000", "out/0001"]);
    let text = emit(&graph).expect("a well-formed graph must emit");
    assert_eq!(
        default_lines(&text)
            .first()
            .map(|line| line.operands.clone()),
        Some(vec!["out/0000", "out/0001", "out/0001"]),
        "a repeated default target must survive the sort as two operands. \
         Emitted text:\n{text}",
    );
}

/// A graph of three edges declaring `targets` as its defaults.
fn graph_declaring(targets: &[&str]) -> BuildGraph {
    let mut graph = BuildGraph::default();
    graph.actions.insert("act".to_owned(), action());
    for index in 0..3 {
        graph
            .insert_edge(edge(index))
            .expect("each edge here owns its own distinct output");
    }
    graph.default_targets = targets.iter().map(Utf8PathBuf::from).collect();
    graph
}
