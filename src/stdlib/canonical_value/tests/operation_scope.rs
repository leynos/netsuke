//! Bounded admission: one budget serves one public operation.
//!
//! The ceilings in `budget` bound a single admission. This battery pins the
//! unit they are spent in: `canonical_eq` and `stable_deduplicate` share one
//! budget across all their operands, so an operation can be refused even when
//! each operand passes `canonical_key` alone. That is deliberate, and a
//! per-value budget would be a different contract rather than a fix.
//!
//! Both cases drive the public surface, so the sharing under test is the
//! shipping arrangement rather than a test-only parallel of it.

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
