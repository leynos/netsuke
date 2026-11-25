//! Control fixture for the mutation-selection contract.
//!
//! This file is parsed by `tests/mutation_selection_contract_tests.rs` and is
//! never compiled: it is not declared as a module, and it is not under `src/`,
//! so neither the crate nor the test harness includes it. Its only job is to be
//! a source shape the contract's reader can be checked against.
//!
//! It holds two same-named pairs, and the pair that matters is the second. In
//! `skipped_kani_half` the skip sits on the `cfg(kani)` implementation, which
//! is the shape `src/ir/graph/mod.rs` uses and the shape the contract accepts.
//! In `skipped_production_half` the skip sits on the `cfg(not(kani))`
//! implementation instead, and no implementation is skipped for the Kani
//! runner.
//!
//! A reader that only asked "does this name carry a skip?" would answer yes to
//! both, and would therefore certify a name-wide exclusion as a per-item one.
//! The contract asserts the two resolve differently, so an implementation that
//! loses that distinction fails rather than passes.

/// A demonstration type standing in for `BuildGraph`.
struct Demo;

impl Demo {
    /// The `cfg(kani)` half of a pair whose skip is on the proof-only half.
    #[cfg(kani)]
    #[mutants::skip]
    fn skipped_kani_half(&self) -> u32 {
        1
    }

    /// The `cfg(not(kani))` half, which stays selected.
    #[cfg(not(kani))]
    fn skipped_kani_half(&self) -> u32 {
        2
    }

    /// The `cfg(kani)` half of a pair whose skip is on the wrong half.
    #[cfg(kani)]
    fn skipped_production_half(&self) -> u32 {
        3
    }

    /// The `cfg(not(kani))` half, wrongly skipped: no mutant of it would run.
    #[cfg(not(kani))]
    #[mutants::skip]
    fn skipped_production_half(&self) -> u32 {
        4
    }
}

/// A block whose skip is on the `impl` rather than on any method.
///
/// cargo-mutants returns from `visit_item_impl` before it reaches the methods,
/// so both methods below are skipped although neither carries an attribute. A
/// reader that inspected only the method's own attributes would report them as
/// selected, which is the discrepancy `the_reader_honours_an_enclosing_impl_skip`
/// exists to catch.
#[mutants::skip]
impl SkippedImpl {
    /// Selected on its own attributes, skipped by the enclosing block.
    fn under_skipped_impl(&self) -> u32 {
        5
    }

    /// Also selected on its own attributes, also skipped by the block.
    fn also_under_skipped_impl(&self) -> u32 {
        6
    }
}

/// A type whose `impl` block carries the skip.
struct SkippedImpl;
