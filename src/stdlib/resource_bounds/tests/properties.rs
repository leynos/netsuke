//! Generated cases for the cardinality contracts, with class coverage.
//!
//! The cases here derive their inputs from a strategy, so one case checks a law
//! over a whole class of requests. The oracles they compare against live in the
//! sibling `oracle` battery and share no code with the kernels.
//!
//! The laws and the coverage tripwires are separate runs on purpose. A law is
//! worth checking against fresh inputs, so the law runners keep `proptest`'s
//! random seed. Coverage is a claim about the strategy rather than about one
//! draw from it, so its runners are pinned to a fixed seed: otherwise an
//! unlucky seed could report a class as missing from a strategy that still
//! emits it, which would be a defect in the test rather than in the code.
//!
//! Every law also repeats its call and compares the two answers. The kernels
//! read no ambient state, so a difference between two calls with the same
//! arguments would mean the function is not a function of its arguments.

use std::cell::Cell;

use anyhow::{Result, anyhow, ensure};
use proptest::prelude::*;
use proptest::test_runner::{Config, RngSeed, TestRunner};

use super::oracle;
use super::*;

/// Cases each law battery runs.
///
/// Pinned rather than left to the default so the workload is the same whatever
/// `PROPTEST_CASES` says in the environment; the sample itself still varies.
const PROPERTY_CASES: u32 = 256;

/// Cases the class-coverage tripwires run.
///
/// Higher than [`PROPERTY_CASES`] because the sample is fixed: a class that
/// survives in the strategy but becomes rare must still show up, and there is
/// no later run to catch what this sample misses.
const COVERAGE_CASES: u32 = 512;

/// Seed for every coverage tripwire's runner.
///
/// Arbitrary and free to change; what matters is that it is fixed, so the
/// verdict and the counts are identical on every run. Its one obligation is
/// that the sample reaches all four classes, which the tripwires check.
const COVERAGE_SEED: u64 = 0x613;

/// The four outcomes a request can produce under a ceiling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Class {
    /// The exact cardinality is zero.
    Zero,
    /// The exact cardinality is at most the ceiling.
    Admitted,
    /// The exact cardinality is above the ceiling but representable.
    Exceeded,
    /// The exact cardinality is above `u64::MAX`.
    Overflow,
}

impl Class {
    /// The whole class set, in the order the tripwires report it.
    const ALL: [Self; 4] = [Self::Zero, Self::Admitted, Self::Exceeded, Self::Overflow];

    /// Classify an exact cardinality against a ceiling.
    ///
    /// Zero is separated from admitted on purpose: it is the class that proves
    /// the absorption and empty-selection rules were reached, and folding it
    /// into "admitted" would make the tripwire blind to a missing zero case.
    const fn of(cardinality: Option<u64>, ceiling: u64) -> Self {
        match cardinality {
            None => Self::Overflow,
            Some(0) => Self::Zero,
            Some(value) if value <= ceiling => Self::Admitted,
            Some(_) => Self::Exceeded,
        }
    }
}

/// How many cases reached each class.
///
/// The cells are what let a tripwire record from inside `proptest`'s closure:
/// `TestRunner::run` takes an `Fn`, not an `FnMut`, so a tally captured by
/// mutable reference would not compile. Interior mutability keeps the counting
/// an implementation detail of the tally rather than a constraint on the case.
#[derive(Debug, Default)]
struct ClassCounts {
    /// Cases whose exact cardinality was zero.
    zero: Cell<usize>,
    /// Cases admitted under the battery's ceiling.
    admitted: Cell<usize>,
    /// Cases refused for exceeding a representable ceiling.
    exceeded: Cell<usize>,
    /// Cases refused because the cardinality is unrepresentable.
    overflow: Cell<usize>,
}

impl ClassCounts {
    /// Record one case.
    fn record(&self, class: Class) {
        let slot = match class {
            Class::Zero => &self.zero,
            Class::Admitted => &self.admitted,
            Class::Exceeded => &self.exceeded,
            Class::Overflow => &self.overflow,
        };
        slot.set(slot.get().saturating_add(1));
    }

    /// Return the count reached for `class`.
    fn reach(&self, class: Class) -> usize {
        let slot = match class {
            Class::Zero => &self.zero,
            Class::Admitted => &self.admitted,
            Class::Exceeded => &self.exceeded,
            Class::Overflow => &self.overflow,
        };
        slot.get()
    }

    /// Fail unless every class was reached at least once.
    fn ensure_all_reached(&self, battery: &str) -> Result<()> {
        for class in Class::ALL {
            ensure!(
                self.reach(class) > 0,
                "{battery} must reach the {class:?} class; counts: {self:?}"
            );
        }
        Ok(())
    }
}

/// A law battery's runner: fixed case count, fresh random seed.
fn law_runner() -> TestRunner {
    TestRunner::new(Config {
        cases: PROPERTY_CASES,
        ..Config::default()
    })
}

/// A coverage tripwire's runner: fixed case count and fixed seed.
fn coverage_runner() -> TestRunner {
    TestRunner::new(Config {
        cases: COVERAGE_CASES,
        rng_seed: RngSeed::Fixed(COVERAGE_SEED),
        ..Config::default()
    })
}

/// Generate one operand length: zero, one, or a power of two up to `2^20`.
///
/// Zero and one are common because they are the two lengths that decide the
/// product before any multiplication runs, which is exactly the branch the
/// witnesses pin and the property must reach independently.
fn operand_length() -> impl Strategy<Value = usize> {
    prop_oneof![
        1 => Just(0_usize),
        1 => Just(1_usize),
        8 => (0_u32..=20).prop_map(|exponent| 1_usize << exponent),
    ]
}

/// Generate up to six operand lengths for a product request.
fn product_request() -> impl Strategy<Value = (Vec<usize>, u64)> {
    (
        prop::collection::vec(operand_length(), 0..=6),
        prop_oneof![1 => Just(0_u64), 1 => Just(1_u64), 6 => 2_u64..=8],
    )
}

/// Generate a combination whose `n` is at most 130.
///
/// One case in eight asks for more items than exist. That is the only way a
/// binomial cardinality is zero, so a strategy confined to `r <= n` would make
/// the `Zero` class unreachable by construction and fail its coverage tripwire
/// against a correct kernel.
fn combination_request() -> impl Strategy<Value = (usize, u64)> {
    (0_usize..=130).prop_flat_map(|length| {
        let width = oracle::width_of(length);
        prop_oneof![
            7 => 0_u64..=width,
            1 => Just(width.saturating_add(1)),
        ]
        .prop_map(move |r| (length, r))
    })
}

/// Generate a permutation whose `n` is at most 40.
///
/// As with [`combination_request`], one case in eight overshoots `n` so the
/// `Zero` class is reachable.
fn permutation_request() -> impl Strategy<Value = (usize, u64)> {
    (0_usize..=40).prop_flat_map(|length| {
        let width = oracle::width_of(length);
        prop_oneof![
            7 => 0_u64..=width,
            1 => Just(width.saturating_add(1)),
        ]
        .prop_map(move |r| (length, r))
    })
}

/// Property: the product kernel equals the checked `u128` oracle.
#[test]
fn product_cardinality_matches_the_oracle() -> Result<()> {
    let mut runner = law_runner();
    runner
        .run(&product_request(), |(lengths, repeat)| {
            let request = CardinalityRequest::Product {
                lengths: &lengths,
                repeat,
            };
            let admitted = request.cardinality();
            prop_assert_eq!(
                admitted,
                oracle::product(&lengths, repeat),
                "the kernel must equal the oracle"
            );
            prop_assert_eq!(
                request.cardinality(),
                admitted,
                "a repeated call must answer the same"
            );
            Ok(())
        })
        .map_err(|error| anyhow!("{error}"))?;
    Ok(())
}

/// Property: the binomial kernel equals the Pascal-triangle oracle.
#[test]
fn combination_cardinality_matches_the_oracle() -> Result<()> {
    let mut runner = law_runner();
    runner
        .run(&combination_request(), |(length, r)| {
            let request = CardinalityRequest::Combinations { length, r };
            let admitted = request.cardinality();
            prop_assert_eq!(
                admitted,
                oracle::binomial(oracle::width_of(length), r),
                "the kernel must equal the oracle"
            );
            prop_assert_eq!(
                request.cardinality(),
                admitted,
                "a repeated call must answer the same"
            );
            Ok(())
        })
        .map_err(|error| anyhow!("{error}"))?;
    Ok(())
}

/// Property: for `r` up to two, the binomial kernel equals the closed form.
///
/// This battery is what covers `u64`-wide `n`, where a Pascal row cannot be
/// built. It is the only case that drives the `u128` intermediate at the top of
/// the range, so it is the case a `u64`-step fault fails.
#[test]
fn combination_cardinality_matches_the_closed_form_for_small_r() -> Result<()> {
    let mut runner = law_runner();
    runner
        .run(&(any::<usize>(), 0_u64..=2), |(length, r)| {
            let request = CardinalityRequest::Combinations { length, r };
            prop_assert_eq!(
                request.cardinality(),
                oracle::binomial_up_to_two(oracle::width_of(length), r),
                "the kernel must equal the closed form"
            );
            Ok(())
        })
        .map_err(|error| anyhow!("{error}"))?;
    Ok(())
}

/// Property: the falling-factorial kernel equals the checked `u128` oracle.
#[test]
fn permutation_cardinality_matches_the_oracle() -> Result<()> {
    let mut runner = law_runner();
    runner
        .run(&permutation_request(), |(length, r)| {
            let request = CardinalityRequest::Permutations { length, r };
            let admitted = request.cardinality();
            prop_assert_eq!(
                admitted,
                oracle::permutations(oracle::width_of(length), r),
                "the kernel must equal the oracle"
            );
            prop_assert_eq!(
                request.cardinality(),
                admitted,
                "a repeated call must answer the same"
            );
            Ok(())
        })
        .map_err(|error| anyhow!("{error}"))?;
    Ok(())
}

/// Tripwire: the product strategy reaches every class.
#[test]
fn product_strategy_reaches_every_class() -> Result<()> {
    let mut runner = coverage_runner();
    let counts = ClassCounts::default();
    runner
        .run(&product_request(), |(lengths, repeat)| {
            let expected = oracle::product(&lengths, repeat);
            counts.record(Class::of(expected, OUTPUT_TUPLE_CEILING));
            prop_assert_eq!(
                CardinalityRequest::Product {
                    lengths: &lengths,
                    repeat
                }
                .cardinality(),
                expected,
                "coverage is only coverage of a correct kernel"
            );
            Ok(())
        })
        .map_err(|error| anyhow!("{error}"))?;
    counts.ensure_all_reached("the product strategy")
}

/// Tripwire: the combination strategy reaches every class.
#[test]
fn combination_strategy_reaches_every_class() -> Result<()> {
    let mut runner = coverage_runner();
    let counts = ClassCounts::default();
    runner
        .run(&combination_request(), |(length, r)| {
            let expected = oracle::binomial(oracle::width_of(length), r);
            counts.record(Class::of(expected, OUTPUT_TUPLE_CEILING));
            prop_assert_eq!(
                CardinalityRequest::Combinations { length, r }.cardinality(),
                expected,
                "coverage is only coverage of a correct kernel"
            );
            Ok(())
        })
        .map_err(|error| anyhow!("{error}"))?;
    counts.ensure_all_reached("the combination strategy")
}

/// Tripwire: the permutation strategy reaches every class.
#[test]
fn permutation_strategy_reaches_every_class() -> Result<()> {
    let mut runner = coverage_runner();
    let counts = ClassCounts::default();
    runner
        .run(&permutation_request(), |(length, r)| {
            let expected = oracle::permutations(oracle::width_of(length), r);
            counts.record(Class::of(expected, PERMUTATION_TUPLE_CEILING));
            prop_assert_eq!(
                CardinalityRequest::Permutations { length, r }.cardinality(),
                expected,
                "coverage is only coverage of a correct kernel"
            );
            Ok(())
        })
        .map_err(|error| anyhow!("{error}"))?;
    counts.ensure_all_reached("the permutation strategy")
}
