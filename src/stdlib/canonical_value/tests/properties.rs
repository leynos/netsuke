//! Property tests over generated `MiniJinja` values.
//!
//! The cases here derive their inputs from a strategy and check a law over the
//! whole sample, so a class of values the generator can reach is covered by one
//! case. The generator constructs admitted values directly rather than
//! filtering arbitrary ones, so no generated case is discarded.
//!
//! The properties run through [`TestRunner`] rather than the `proptest!` macro,
//! because the macro cannot report per-class counts back to the caller and the
//! generator's coverage has to be asserted rather than assumed.
//!
//! The batteries split by what they claim about the sample, and the two claims
//! want opposite things from it. A law is worth checking against fresh inputs,
//! so the law runners keep proptest's random seed. Coverage is a claim about
//! the generator rather than about one draw from it, so its runner is pinned to
//! a fixed seed: otherwise an unlucky seed could report a class as missing from
//! a generator that still emits it, which is a defect in the test rather than
//! in the generator. The two are separate tests for that reason.

use std::cell::RefCell;
use std::collections::BTreeMap;

use anyhow::{Result, ensure};
use minijinja::value::{Value, ValueKind};
use proptest::prelude::*;
use proptest::test_runner::{Config, RngSeed, TestRunner};

use super::*;

/// Maximum nesting depth the recursive generator builds.
const GENERATED_DEPTH: u32 = 3;

/// Cases each law battery runs.
///
/// Pinned rather than left to the default so the workload is the same whatever
/// `PROPTEST_CASES` says in the environment. The sample itself still varies:
/// the law batteries keep proptest's random seed, because a law is worth
/// checking against inputs no earlier run has tried.
const PROPERTY_CASES: u32 = 64;

/// Cases the generator-coverage tripwire runs.
///
/// Set well above [`PROPERTY_CASES`] because the sample is fixed: a class that
/// survives in the pool but becomes rare must still show up in the count, and
/// there is no later run to catch what this sample misses.
const COVERAGE_CASES: u32 = 512;

/// Seed for the coverage tripwire's runner.
///
/// Arbitrary, and free to be changed: what matters is that it is fixed, so the
/// verdict and the counts it reports are identical on every run. Its one
/// obligation is that the sample it draws reaches all seven material classes,
/// which the tripwire itself checks.
const COVERAGE_SEED: u64 = 0x612;

/// The exponent bits of an IEEE-754 binary64.
const EXPONENT_MASK: u64 = 0x7ff0_0000_0000_0000;

/// A finite exponent: unbiased zero, so the generator covers the normal range
/// either side of 1.0 without ever reaching a non-finite value.
const FINITE_EXPONENT: u64 = 0x3ff_u64 << 52;

/// Weight given to a scalar relative to one container at each level.
///
/// `prop_recursive` decides per value whether to recurse, so an even split
/// between scalars and containers starves the classes that only appear in the
/// scalar pool: a scalar is chosen at a level only when the coin says so, and
/// the containers then dominate the sample. Weighting the scalars keeps every
/// pool member reachable at the top level.
const SCALAR_WEIGHT: u32 = 8;

/// A `TestRunner` pinned to [`PROPERTY_CASES`] cases and a fresh random seed.
///
/// The seed is deliberately left free, so each run tries inputs no earlier run
/// has tried. Only the case count is fixed.
pub(super) fn law_runner() -> TestRunner {
    TestRunner::new(Config {
        cases: PROPERTY_CASES,
        ..Config::default()
    })
}

/// A `TestRunner` pinned to [`COVERAGE_CASES`] cases and to a fixed seed.
///
/// The fixed seed is the point of this runner. A coverage assertion is a claim
/// about the generator, not about one draw from it, so it must not be able to
/// fail because an unlucky seed under-sampled a class. Freezing the sample
/// makes the verdict and the reported counts identical on every run, which is
/// what lets the counts be compared between runs at all.
pub(super) fn coverage_runner() -> TestRunner {
    TestRunner::new(Config {
        cases: COVERAGE_CASES,
        rng_seed: RngSeed::Fixed(COVERAGE_SEED),
        ..Config::default()
    })
}

/// Generate one admitted value, recursing into sequences and mappings.
///
/// The strategy constructs admitted values directly rather than filtering
/// arbitrary ones, so no generated case is discarded. Scalars are drawn from a
/// narrow pool on purpose — `None`, a boolean, an integer, a finite float, a
/// lowercase string, and two astral strings — so every one of them is reached
/// often enough to be counted, which a wide pool would prevent.
///
/// The scalar pool is boxed before recursing because its arms have eight
/// different types, and `prop_recursive` requires one strategy for the base
/// case rather than a union of them.
pub(super) fn generated_values() -> BoxedStrategy<Value> {
    let scalar = prop_oneof![
        Just(Value::from(())),
        any::<bool>().prop_map(Value::from),
        any::<i64>().prop_map(Value::from),
        generated_finite().prop_map(Value::from),
        "[a-z]{0,6}".prop_map(Value::from),
        Just(Value::from("😀")),
        Just(Value::from("\u{fb33}")),
        Just(Value::from(-0.0)),
    ]
    .boxed();
    scalar
        .prop_recursive(GENERATED_DEPTH, 16, 4, |inner| {
            prop_oneof![
                SCALAR_WEIGHT => inner.clone(),
                3 => prop::collection::vec(inner.clone(), 0..3).prop_map(Value::from),
                3 => prop::collection::btree_map("x{0,3}", inner, 0..3)
                    .prop_map(|entries| map_of(entries.into_iter().collect())),
            ]
        })
        .boxed()
}

/// Generate a finite binary64 from uniformly random bits.
///
/// The exponent field is replaced with a finite one, so the value can never be
/// `NaN` or infinite. That keeps the generator constructive rather than
/// filtering: a rejected case would silently skew the classification counts,
/// and a rejection rate near one half would exhaust the reject budget. The
/// exponent is fixed rather than masked because a masked exponent is almost
/// always subnormal, which would leave the ordinary range uncovered.
fn generated_finite() -> BoxedStrategy<f64> {
    any::<u64>()
        .prop_map(|bits| f64::from_bits((bits & !EXPONENT_MASK) | FINITE_EXPONENT))
        .boxed()
}

/// Verify the four equivalence-relation laws for generated values.
///
/// Reflexivity, symmetry, and transitivity are checked, the last against
/// independently rebuilt copies of the same content. Coverage of the generator
/// is a separate claim with a separate test below: a law and a generator
/// property want opposite things from the sample, so one test cannot carry
/// both without making the coverage verdict depend on the draw.
#[test]
fn generated_values_obey_the_equivalence_relation() -> Result<()> {
    let mut runner = law_runner();
    runner
        .run(&generated_values(), |value| {
            let key = match canonical_key(&value) {
                Ok(key) => key,
                Err(error) => return Err(TestCaseError::fail(error.to_string())),
            };
            check_laws(&value, &key).map_err(|error| TestCaseError::fail(format!("{error:#}")))
        })
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    Ok(())
}

/// Assert the generator can still reach every material class, and report the
/// classification counts.
///
/// This is the non-vacuity guard for the batteries above: a generator that
/// stopped emitting, say, booleans would leave the law test passing over a
/// sample it never made. It runs on its own frozen sample for that reason, and
/// over more cases than the law battery, since a class that becomes rare must
/// be caught here rather than hoped for there.
#[test]
fn generated_values_reach_every_material_class() -> Result<()> {
    let counts: RefCell<BTreeMap<&'static str, usize>> = RefCell::new(BTreeMap::new());
    let mut runner = coverage_runner();
    runner
        .run(&generated_values(), |value| {
            *counts
                .borrow_mut()
                .entry(material_class(&value))
                .or_insert(0) += 1;
            Ok(())
        })
        .map_err(|error| anyhow::anyhow!("{error}"))?;

    let tally = counts.into_inner();
    for class in REQUIRED_CLASSES {
        ensure!(
            tally.get(class).copied().unwrap_or(0) > 0,
            "class `{class}` was never generated in {COVERAGE_CASES} cases \
             at seed {COVERAGE_SEED}: {tally:?}",
        );
    }
    Ok(())
}

/// Check reflexivity, symmetry, transitivity, and key stability.
///
/// A rebuilt copy of the same content stands in for the independently built
/// third value transitivity needs, and it is rebuilt again for the second
/// relation in the chain, so no step reuses the first comparison's operands.
///
/// The chain states both premises and then their conclusion. Checking the
/// premises alone would establish nothing about the endpoints: `value ≠ twice`
/// is consistent with two true premises whenever the relation is not
/// transitive, so the conclusion is asserted rather than inferred.
fn check_laws(value: &Value, key: &CanonicalKey) -> Result<()> {
    let rebuilt = rebuild(value);
    let twice = rebuild(&rebuilt);

    ensure!(
        canonical_eq(value, value).context("an admitted value is comparable with itself")?,
        "an admitted value must equal itself",
    );
    ensure!(
        canonical_eq(value, &rebuilt).context("a rebuilt copy is admitted")?,
        "a rebuilt copy must compare equal",
    );
    ensure!(
        canonical_eq(&rebuilt, &twice).context("a twice-rebuilt copy is admitted")?,
        "transitivity must hold across independent rebuilds",
    );
    ensure!(
        canonical_eq(value, &twice).context("the twice-rebuilt copy is admitted")?,
        "transitivity must conclude: both premises hold, so the endpoints compare equal",
    );
    ensure!(
        canonical_eq(&rebuilt, value).context("the rebuilt copy is admitted")?,
        "equality must be symmetric",
    );
    ensure!(
        !canonical_eq(value, &Value::from("unlike")).context("an unlike value is admitted")?,
        "no value may equal an unlike value",
    );
    ensure!(
        canonical_key(value).context("second derivation")? == *key,
        "deriving a key twice must agree",
    );
    Ok(())
}

/// Name the generator class `value` belongs to, for the coverage tally.
fn material_class(value: &Value) -> &'static str {
    match value.kind() {
        ValueKind::None => "null",
        ValueKind::Bool => "bool",
        ValueKind::Number if value.is_integer() => "int",
        ValueKind::Number => "float",
        ValueKind::String => "string",
        ValueKind::Seq => "seq",
        ValueKind::Map => "map",
        _ => "other",
    }
}

/// Rebuild a value through an independent construction of the same content.
///
/// Sequences are rebuilt as a plain list, which is a different host type from
/// the generated one. Mappings are rebuilt by reading each member through
/// `get_attr` rather than through the enumerator, so the value side of the
/// rebuild does not reuse the traversal that produced the key.
fn rebuild(value: &Value) -> Value {
    match value.kind() {
        ValueKind::Seq => {
            let items: Vec<Value> = value
                .try_iter()
                .map(|iter| iter.map(|item| rebuild(&item)).collect())
                .unwrap_or_default();
            Value::from(items)
        }
        ValueKind::Map => {
            let names: Vec<String> = value
                .try_iter()
                .map(|iter| {
                    iter.filter_map(|key| key.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default();
            map_of(
                names
                    .into_iter()
                    .map(|name| {
                        let member = value.get_attr(&name).unwrap_or_default();
                        (name, rebuild(&member))
                    })
                    .collect(),
            )
        }
        _ => value.clone(),
    }
}

/// Property: deduplication keeps exactly one representative per canonical key
/// for generated sequences.
///
/// This case counts and de-duplicates; it says nothing about the order the
/// representatives come back in, nor about which of two equal values was kept.
/// Both are checked by the independent oracle in the sibling `oracle` battery,
/// whose [`deduplication_agrees_with_the_independent_oracle`] fixes the retained
/// values' identity and order against a quadratic first-index expectation.
#[test]
fn deduplication_keeps_the_first_representative() -> Result<()> {
    let mut runner = law_runner();
    runner
        .run(&prop::collection::vec(generated_values(), 0..8), |values| {
            let retained = stable_deduplicate(&values)
                .map_err(|error| TestCaseError::fail(error.to_string()))?;
            let keys: Vec<CanonicalKey> = retained
                .iter()
                .map(|value| {
                    canonical_key(value).map_err(|error| TestCaseError::fail(error.to_string()))
                })
                .collect::<Result<Vec<_>, _>>()?;
            prop_assert!(distinct(&keys), "retained keys must be distinct");
            let expected =
                distinct_count(&values).map_err(|error| TestCaseError::fail(error.to_string()))?;
            prop_assert_eq!(
                retained.len(),
                expected,
                "one representative per distinct key"
            );
            Ok(())
        })
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    Ok(())
}

/// Report whether a key slice holds no repeats.
fn distinct(keys: &[CanonicalKey]) -> bool {
    let seen: std::collections::HashSet<&CanonicalKey> = keys.iter().collect();
    seen.len() == keys.len()
}

/// Count the distinct canonical keys in `values`, refusing any inadmissible value.
///
/// A refusal propagates rather than being filtered away. A count that dropped
/// the values it could not key would compare the deduplicated length against a
/// shrunken expectation, so a generator that started emitting refusals would
/// weaken the property below instead of failing it.
pub(super) fn distinct_count(values: &[Value]) -> std::result::Result<usize, CanonicalValueError> {
    let keys = values
        .iter()
        .map(canonical_key)
        .collect::<std::result::Result<std::collections::HashSet<CanonicalKey>, _>>()?;
    Ok(keys.len())
}
