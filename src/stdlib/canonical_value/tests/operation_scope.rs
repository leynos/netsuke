//! Bounded admission: one budget serves one public operation.
//!
//! The ceilings in `budget` bound a single admission. This battery pins the
//! unit they are spent in: `canonical_eq` and `stable_deduplicate` share one
//! budget across all their operands, so an operation can be refused even when
//! each operand passes `canonical_key` alone. That is deliberate, and a
//! per-value budget would be a different contract rather than a fix.
//!
//! The cases come in two pairs. The `*_within` pair uses a four-node budget so
//! the sharing is visible on a trivial fixture; the production pair drives the
//! exported `canonical_eq` and `stable_deduplicate` with operands that each fit
//! the real 100,000-node ceiling and exceed it together. The second pair is
//! what would fail if a public wrapper quietly gave each operand its own
//! budget, which the explicit-budget pair cannot see.

use anyhow::{Result, ensure};
use minijinja::value::Value;
use rstest::rstest;

use super::*;

/// The public operation whose budget is shared across its operands.
#[derive(Clone, Copy, Debug)]
enum SharedOperation {
    /// `canonical_eq`, which compares two operands.
    Equality,
    /// `stable_deduplicate`, which reads every input.
    Deduplication,
}

/// Build two two-item sequences that each fit a four-node budget alone.
///
/// Asserting the premise here rather than in each case is what makes the
/// refusal below evidence of sharing: without it, an operand that could not be
/// admitted on its own would refuse for the wrong reason.
fn two_sequences_each_within_four_nodes() -> Result<[Value; 2]> {
    let pair = [Value::from([1_i64, 2]), Value::from([1_i64, 2])];
    for value in &pair {
        ensure!(
            canonical_key_within(value, &mut admission_budget(100, 4, 100)).is_ok(),
            "each input must fit the four-node ceiling on its own",
        );
    }
    Ok(pair)
}

/// One budget serves one public operation rather than one value.
#[rstest]
#[case::equality(SharedOperation::Equality)]
#[case::deduplication(SharedOperation::Deduplication)]
fn one_budget_is_shared_across_the_operation(#[case] operation: SharedOperation) -> Result<()> {
    let inputs = two_sequences_each_within_four_nodes()?;
    let mut budget = admission_budget(100, 4, 100);
    let refusal = match operation {
        SharedOperation::Equality => canonical_eq_within(&inputs[0], &inputs[1], &mut budget)
            .expect_err("one budget cannot admit both operands"),
        SharedOperation::Deduplication => stable_deduplicate_within(&inputs, &mut budget)
            .expect_err("one budget cannot admit both inputs"),
    };
    ensure_resource_refusal(&refusal, CanonicalResource::Nodes)
}

/// Depth of the shared chain whose two copies exceed the production budget.
///
/// One chain is `2^(depth + 1) - 1 = 65,535` occurrences, which the production
/// node ceiling of 100,000 admits; two are 131,070, which it does not. The
/// chain is 15 edges deep, well inside the 128-edge ceiling, and charges no
/// bytes, so the node ceiling is the only one in play.
const SPLIT_BUDGET_DEPTH: usize = 15;

/// Build a chain of `depth` two-item sequences whose items share one child.
///
/// Every level holds two clones of the same child, so the value is `depth + 1`
/// host objects standing for `2^(depth + 1) - 1` occurrences.
fn shared_chain(depth: usize) -> Value {
    (0..depth).fold(Value::from(0_i64), |child, _| {
        Value::from(vec![child.clone(), child])
    })
}

/// Build two shared chains that each fit the production ceilings alone.
///
/// Asserting the premise through the exported `canonical_key` is what makes the
/// refusals below evidence of sharing: without it, an operand that could not be
/// admitted on its own would refuse for the wrong reason.
fn two_chains_each_within_the_production_ceiling() -> Result<[Value; 2]> {
    let pair: [Value; 2] = std::array::from_fn(|_| shared_chain(SPLIT_BUDGET_DEPTH));
    for chain in &pair {
        ensure!(
            canonical_key(chain).is_ok(),
            "each chain must fit the production node ceiling on its own",
        );
    }
    Ok(pair)
}

/// `canonical_eq` spends one production budget across both operands.
#[rstest]
fn canonical_eq_shares_the_production_budget() -> Result<()> {
    let [left, right] = two_chains_each_within_the_production_ceiling()?;
    let refusal = canonical_eq(&left, &right)
        .expect_err("131,070 occurrences exceed the production node ceiling");
    ensure_resource_refusal(&refusal, CanonicalResource::Nodes)
}

/// `stable_deduplicate` spends one production budget across every input.
#[rstest]
fn stable_deduplicate_shares_the_production_budget() -> Result<()> {
    let inputs = two_chains_each_within_the_production_ceiling()?;
    let refusal = stable_deduplicate(&inputs)
        .expect_err("131,070 occurrences exceed the production node ceiling");
    ensure_resource_refusal(&refusal, CanonicalResource::Nodes)
}
