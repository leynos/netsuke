//! Diagnostic rendering: the message each refusal produces, pinned exactly.
//!
//! The sibling `boundaries` battery asserts *which* refusal is raised and which
//! kind it names, and checks the message through `contains` fragments. A
//! fragment check survives a reworded message, a dropped interpolation, or a
//! location that quietly moved, because it asks only whether some substring is
//! still present. This battery pins the whole rendered message instead, so a
//! change to either the wording or the path rendering fails here rather than
//! passing unnoticed.
//!
//! The path renderer has three branches — the root marker, the short form, and
//! a middle-elided form — and only the first two were reachable from any
//! existing case: no other test builds a path longer than the elision limit, so
//! the elided branch and the boundary that selects it were untested. The cases
//! either side of that boundary are snapshotted here, which is what makes a
//! change to the limit or to the elision wording visible.
//!
//! Typed and semantic assertions are kept alongside the snapshots rather than
//! replaced by them: a snapshot says what the message is, not that the message
//! belongs to the right variant.

use anyhow::{Result, ensure};
use insta::assert_snapshot;
use minijinja::value::{Value, ValueKind};
use rstest::rstest;

use super::*;

use crate::snapshot_test_support::snapshot_settings;

/// Assert a refusal renders `name`'s snapshot exactly.
///
/// Binding through [`snapshot_settings`] roots every snapshot under
/// `src/snapshots/canonical_value/`, so the set stays together and a later
/// battery cannot invent a second location for the same subject.
fn assert_rendered(name: &str, refusal: &CanonicalValueError) {
    let rendered = refusal.to_string();
    snapshot_settings("canonical_value").bind(|| {
        assert_snapshot!(name, rendered);
    });
}

/// Build a value whose refusal path is `depth` distinctly named members deep.
///
/// The fixture is local to this battery because it exists to reach one branch:
/// the elision fires only past a limit no other battery's fixtures approach, so
/// a shared copy would be a fixture with a single caller kept somewhere it does
/// not belong.
///
/// Each level is named for its own depth rather than reusing [`nested_map`]'s
/// single member name. Reusing one name would render every edge identically, so
/// an elided path's head and tail would be the same text and a renderer that
/// emitted them in the wrong order — or dropped one end entirely — would still
/// match the snapshot. Distinct names are what make the two ends
/// distinguishable, and therefore what makes the boundary snapshots evidence.
fn member_path_of_depth(depth: usize, leaf: Value) -> Value {
    (0..depth).fold(leaf, |inner, level| {
        map_of(vec![(format!("m{level:02}"), inner)])
    })
}

/// Pin the message for a value outside the canonical-JSON domain.
#[rstest]
fn unsupported_kind_message_is_pinned() -> Result<()> {
    let refusal = canonical_key(&Value::from_bytes(vec![1])).expect_err("bytes are refused");
    ensure_refused(&Value::from_bytes(vec![1]), ValueKind::Bytes, "bytes")?;
    assert_rendered("unsupported_kind", &refusal);
    Ok(())
}

/// Pin the message for a mapping key that is not a string.
#[rstest]
fn non_string_key_message_is_pinned() -> Result<()> {
    let (probe, _) = fixture(
        ObjectRepr::Map,
        Some(1),
        Materialized::Pairs(vec![(Value::from(1), Value::from("one"))]),
    );
    let refusal = canonical_key(&probe).expect_err("an integer key is refused");
    ensure!(
        refusal.kind() == Some(ValueKind::Number),
        "the refusal must name the key's kind, got {refusal}",
    );
    assert_rendered("non_string_key", &refusal);
    Ok(())
}

/// Pin the message for a non-finite float.
#[rstest]
fn non_finite_message_is_pinned() -> Result<()> {
    let refusal = canonical_key(&Value::from(f64::NAN)).expect_err("NaN is refused");
    ensure!(
        refusal.kind().is_none(),
        "a non-finite refusal names no value kind, got {refusal}",
    );
    assert_rendered("non_finite_number", &refusal);
    Ok(())
}

/// Pin the message for a host value that cannot be traversed as its kind.
#[rstest]
fn incompatible_representation_message_is_pinned() -> Result<()> {
    let (probe, _) = fixture(ObjectRepr::Map, None, Materialized::NonEnumerable);
    let refusal = canonical_key(&probe).expect_err("a non-enumerable map is refused");
    ensure!(
        matches!(
            refusal,
            CanonicalValueError::IncompatibleRepresentation { .. }
        ),
        "expected an untraversable refusal, got {refusal}",
    );
    assert_rendered("incompatible_representation", &refusal);
    Ok(())
}

/// Pin the message for a canonicalizer refusal.
///
/// This variant is the one case here that is constructed rather than raised.
/// Its documented contract is that the snapshot tree admits nothing the
/// canonicalizer rejects and holds no non-string keys, so no admitted input can
/// reach it and no call through the public surface can produce it. Its
/// rendering is still part of the public surface, because the enum is public
/// and a caller may match the variant, so the message is pinned from a directly
/// constructed value and this note records why the case cannot drive the
/// adapter.
#[rstest]
fn serialization_message_is_pinned() -> Result<()> {
    let source = serde_json::from_str::<serde_json::Value>("{")
        .expect_err("an unterminated object is not valid JSON");
    let refusal = CanonicalValueError::Serialization { source };
    ensure!(
        refusal.kind().is_none(),
        "a serialization refusal names no value kind, got {refusal}",
    );
    assert_rendered("serialization", &refusal);
    Ok(())
}

/// Pin the longest path that still renders in full.
///
/// The elision limit is a documented boundary, and a snapshot immediately below
/// it is what makes an off-by-one change to the comparison visible: a path at
/// the limit must keep every edge.
#[rstest]
fn path_at_the_render_limit_is_not_elided() -> Result<()> {
    let refusal =
        canonical_key(&member_path_of_depth(8, Value::from_bytes(vec![1]))).expect_err("refused");
    ensure!(
        !refusal.to_string().contains("more)"),
        "a path at the limit must not be abbreviated, got {refusal}",
    );
    assert_rendered("path_at_limit", &refusal);
    Ok(())
}

/// Pin the shortest path whose middle is elided.
///
/// The case directly past the limit is the other half of the boundary: the
/// comparison that selects the elided branch is exercised here and only here,
/// so a change that shifted it by one would fail this snapshot and the one
/// above together.
#[rstest]
fn path_past_the_render_limit_elides_its_middle() -> Result<()> {
    let refusal =
        canonical_key(&member_path_of_depth(9, Value::from_bytes(vec![1]))).expect_err("refused");
    let rendered = refusal.to_string();
    ensure!(
        rendered.contains("more)"),
        "a path past the limit must be abbreviated, got {rendered}",
    );
    ensure!(
        rendered.starts_with("value of kind `"),
        "the message must still name what was refused, got {rendered}",
    );
    assert_rendered("path_elided_once", &refusal);
    Ok(())
}

/// Pin an elided path that omits more than the minimum.
///
/// The count of omitted edges is interpolated rather than fixed, so a path well
/// past the limit is what shows the number is computed from the path's length
/// instead of being a constant that happens to be right at one depth.
///
/// The expectation is derived arithmetically from the fixture's depth and the
/// renderer's stated edge budget, rather than read back out of the recorded
/// snapshot. A snapshot does not catch a changed count on its own: it compares
/// against bytes that were themselves accepted at some point, so a blind
/// re-record launders a wrong number into a passing baseline. The derived
/// expectation is re-checked on every run regardless of when the snapshot was
/// last refreshed, and fails naming the number it wanted.
#[rstest]
fn deeper_paths_report_every_omitted_edge() -> Result<()> {
    const DEPTH: usize = 12;
    // Six, not "whatever the renderer retains": three edges survive at each end
    // of an elided path. This is deliberately a literal rather than a reference
    // to the renderer's own `RENDER_EDGE` constant. Reading the constant would
    // make the expectation follow a changed budget silently, whereas a literal
    // fails and names the number it wanted, so a widened budget reaches review
    // as a failing test instead of as a re-recorded snapshot.
    const RETAINED_EDGES: usize = 6;
    let refusal = canonical_key(&member_path_of_depth(DEPTH, Value::from_bytes(vec![1])))
        .expect_err("refused");
    let rendered = refusal.to_string();
    ensure!(
        rendered.contains(&format!("({} more)", DEPTH - RETAINED_EDGES)),
        "a {DEPTH}-edge path must report {} omitted edges, got {rendered}",
        DEPTH - RETAINED_EDGES,
    );
    assert_rendered("path_elided_deeply", &refusal);
    Ok(())
}

// ---------------------------------------------------------------------------
// Resource-limit refusals
// ---------------------------------------------------------------------------

/// Build a shared map graph whose member names differ at every level.
///
/// Two members of each level hold clones of one child, so the expansion is
/// exponential in the depth, and naming the pair after its level keeps an
/// elided path's head and tail distinguishable in the snapshot.
fn shared_named_maps(depth: usize) -> Value {
    (0..depth).fold(Value::from(0_i64), |child, level| {
        let (node, _) = fixture(
            ObjectRepr::Map,
            Some(2),
            Materialized::Pairs(vec![
                (Value::from(format!("m{level:02}")), child.clone()),
                (Value::from(format!("m{level:02}b")), child),
            ]),
        );
        node
    })
}

/// Pin the depth refusal for a path that renders in full.
#[rstest]
fn resource_depth_short_message_is_pinned() -> Result<()> {
    let refusal = canonical_key_within(
        &member_path_of_depth(2, Value::from(1_i64)),
        &mut admission_budget(1, 100, 100),
    )
    .expect_err("the leaf sits two edges below a one-edge ceiling");
    ensure_resource_refusal(&refusal, CanonicalResource::Depth)?;
    assert_rendered("resource_depth_short", &refusal);
    Ok(())
}

/// Pin the depth refusal for a path whose middle is elided.
///
/// The depth is one past the production ceiling recorded in the developers'
/// guide, so this case also shows that the default budget is the one stated
/// there. The literal is deliberate: reading the constant would let a changed
/// default pass silently.
#[rstest]
fn resource_depth_elided_message_is_pinned() -> Result<()> {
    let refusal = canonical_key(&member_path_of_depth(129, Value::from(1_i64)))
        .expect_err("the leaf is one edge past a 128-edge ceiling");
    ensure_resource_refusal(&refusal, CanonicalResource::Depth)?;
    let rendered = refusal.to_string();
    ensure!(
        rendered.contains("more)"),
        "a 129-edge path must be abbreviated, got {rendered}",
    );
    assert_rendered("resource_depth_elided", &refusal);
    Ok(())
}

/// Pin the node refusal for the root of a flat sequence.
#[rstest]
fn resource_nodes_short_message_is_pinned() -> Result<()> {
    let refusal = canonical_key_within(
        &Value::from([1_i64, 2, 3]),
        &mut admission_budget(100, 3, 100),
    )
    .expect_err("four nodes exceed a three-node ceiling");
    ensure_resource_refusal(&refusal, CanonicalResource::Nodes)?;
    assert_rendered("resource_nodes_short", &refusal);
    Ok(())
}

/// Pin the node refusal reached through a shared graph.
///
/// The graph is the same shape as the reported trigger: far more occurrences
/// than objects. Its refusal path is deep enough to be elided, which is why
/// this case, rather than the flat one above, covers the elided node message.
#[rstest]
fn resource_nodes_elided_message_is_pinned() -> Result<()> {
    let refusal = canonical_key(&shared_named_maps(64))
        .expect_err("the expansion exceeds the production node ceiling");
    ensure_resource_refusal(&refusal, CanonicalResource::Nodes)?;
    let rendered = refusal.to_string();
    ensure!(
        rendered.contains("more)"),
        "a refusal reached through sharing must be abbreviated, got {rendered}",
    );
    assert_rendered("resource_nodes_elided", &refusal);
    Ok(())
}

/// Pin the byte refusal for a scalar at the root.
#[rstest]
fn resource_bytes_short_message_is_pinned() -> Result<()> {
    let refusal = canonical_key_within(&Value::from("abc"), &mut admission_budget(100, 100, 2))
        .expect_err("three bytes exceed a two-byte ceiling");
    ensure_resource_refusal(&refusal, CanonicalResource::Bytes)?;
    assert_rendered("resource_bytes_short", &refusal);
    Ok(())
}

/// Pin the byte refusal for a member name chain long enough to be elided.
#[rstest]
fn resource_bytes_elided_message_is_pinned() -> Result<()> {
    let refusal = canonical_key_within(
        &member_path_of_depth(12, Value::from("abcdefgh")),
        &mut admission_budget(100, 100, 40),
    )
    .expect_err("the member names and leaf exceed a forty-byte ceiling");
    ensure_resource_refusal(&refusal, CanonicalResource::Bytes)?;
    let rendered = refusal.to_string();
    ensure!(
        rendered.contains("more)"),
        "a deep byte refusal must be abbreviated, got {rendered}",
    );
    assert_rendered("resource_bytes_elided", &refusal);
    Ok(())
}
