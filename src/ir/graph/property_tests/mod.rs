//! Property tests for the IR obligations `ADR-004` defers to larger N.
//!
//! Two obligations live here, both about what `from_manifest` *rejects*:
//! [`duplicates`] covers duplicate outputs and [`cycles`] covers dependency
//! cycles. Kani proves each for a fixed minimal manifest; these extend the
//! same claims to a generated domain of up to thirty targets. That extension
//! is the whole point — the failure classes worth finding are the ones a
//! bounded harness cannot reach, such as a collision between two targets'
//! outputs rather than two outputs of one target, or a cycle longer than the
//! harness's unwinding bound.
//!
//! The manifest is only a carrier for the shape each obligation cares about,
//! so each suite builds its own: the duplication suite varies output names and
//! target count, and the cycle suite varies the dependency relation. The
//! builders are deliberately separate — a target's output and its
//! dependencies are the whole difference between the two suites, and folding
//! them together would hide which knob each one turns.
//!
//! The `property_tests` path segment is load-bearing: `make proptest` selects
//! by a module segment ending in `proptests` or `property_tests`, so a suite
//! filed elsewhere would never run under the gate while the gate still
//! exited 0.

pub(super) mod cycles;
pub(super) mod duplicates;
