//! Snapshots pinning each refusal's rendered message.
//!
//! A rendering is checked twice over, on purpose. The typed assertions read the
//! fields, so a snapshot cannot pass for the wrong variant or for a variant
//! whose numbers were swapped; the snapshot then pins the exact sentence,
//! including the integer formatting of a `u64::MAX` value, which no field
//! assertion would notice. Neither half subsumes the other, so both are kept.
//!
//! The messages are English and developer-facing. They are not Fluent keys and
//! carry no `netsuke::jinja::` code (D-03): the consumer task owns localised
//! text, and `Display` output must never be forwarded into a template error.

use googletest::prelude::*;

use crate::snapshot_test_support::snapshot_settings;

use super::*;

/// Compare one refusal's rendered text against its committed snapshot.
fn assert_rendered(name: &str, rendered: &str) {
    snapshot_settings("resource_bounds").bind(|| {
        insta::assert_snapshot!(name, rendered);
    });
}

/// An over-ceiling refusal names the cardinality and the ceiling.
#[test]
fn cardinality_exceeded_names_both_numbers() {
    let refusal = CardinalityError::Exceeded {
        cardinality: 100_172,
        ceiling: OUTPUT_TUPLE_CEILING,
    };
    let rendered = refusal.to_string();
    let CardinalityError::Exceeded {
        cardinality,
        ceiling,
    } = refusal
    else {
        panic!("the fixture must be the exceeded variant");
    };
    assert_that!(cardinality, eq(100_172));
    assert_that!(ceiling, eq(OUTPUT_TUPLE_CEILING));
    assert_rendered("cardinality_exceeded", &rendered);
}

/// An overflow refusal names the largest representable count exactly.
///
/// The rendered `u64::MAX` is the case that would break under a floating-point
/// formatter, which is why it is snapshotted rather than only compared.
#[test]
fn cardinality_overflow_states_the_largest_count() {
    let refusal = CardinalityError::Overflow {
        ceiling: OUTPUT_TUPLE_CEILING,
    };
    let rendered = refusal.to_string();
    let CardinalityError::Overflow { ceiling } = refusal else {
        panic!("the fixture must be the overflow variant");
    };
    assert_that!(ceiling, eq(OUTPUT_TUPLE_CEILING));
    assert_rendered("cardinality_overflow", &rendered);
}

/// An input-length refusal names the measured byte length.
#[test]
fn input_length_exceeded_names_the_bytes() {
    let refusal = InputLengthExceeded {
        length: u64::MAX,
        ceiling: INPUT_LENGTH_CEILING,
    };
    let rendered = refusal.to_string();
    assert_that!(refusal.length, eq(u64::MAX));
    assert_that!(refusal.ceiling, eq(INPUT_LENGTH_CEILING));
    assert_rendered("input_length_exceeded", &rendered);
}

/// A depth refusal names the attempted depth.
#[test]
fn depth_exceeded_names_the_depth() {
    let refusal = DepthExceeded {
        depth: 129,
        ceiling: 128,
    };
    let rendered = refusal.to_string();
    assert_that!(refusal.depth, eq(129_u32));
    assert_that!(refusal.ceiling, eq(128_u32));
    assert_rendered("depth_exceeded", &rendered);
}

/// A match-count refusal names the ceiling, and no running total.
///
/// The refusal deliberately has no total field: a stream stops at the first
/// excess item, so any total it could report would be an estimate presented as
/// a count (D-08).
#[test]
fn match_count_refusal_names_the_ceiling() {
    let refusal = StreamLimitExceeded {
        kind: StreamKind::MatchCount,
        ceiling: MATCH_COUNT_CEILING,
    };
    let rendered = refusal.to_string();
    assert_that!(refusal.kind, eq(StreamKind::MatchCount));
    assert_that!(refusal.ceiling, eq(MATCH_COUNT_CEILING));
    assert_rendered("stream_match_count", &rendered);
}

/// An output-byte refusal renders its own sentence for the same shape.
#[test]
fn output_bytes_refusal_names_the_ceiling() {
    let refusal = StreamLimitExceeded {
        kind: StreamKind::OutputBytes,
        ceiling: OUTPUT_BYTES_CEILING,
    };
    let rendered = refusal.to_string();
    assert_that!(refusal.kind, eq(StreamKind::OutputBytes));
    assert_that!(refusal.ceiling, eq(OUTPUT_BYTES_CEILING));
    assert_rendered("stream_output_bytes", &rendered);
}
