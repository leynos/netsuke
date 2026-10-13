//! Contract tests keeping Kani mutation evidence in lockstep with harnesses.
//!
//! Each `#[kani::proof]` harness demonstrates it can fail via a mutation
//! patch under `docs/verification/mutations/` that seeds one realistic fault
//! into the production code the harness drives. These tests keep that
//! evidence honest outside the Kani runner:
//!
//! - every patch must still apply to the current tree (`git apply --check`),
//!   catching silent rot when production code near a patched hunk moves;
//! - every harness must own a correspondingly named patch, or appear in an
//!   explicit exemption list with a stated reason; and
//! - every patch must correspond to a live harness, catching renames.
//!
//! Patch names use the harness path with `::` replaced by `__`, for example
//! `ir__cycle__verification__self_dependency_reports_cycle.patch`.
//!
//! A patch that applies is not yet trustworthy: the tree it produces must also
//! compile, or `cargo kani` never reaches the harness it targets. That second
//! half lives in [`compile_guard`], which is expensive enough to be gated.

// This target is a crate root, so `mod child;` would resolve against
// `tests/`; the children live beside this file. The repository's other
// directory-module test trees select one child with `#[path]` and let the
// rest resolve normally, which is available to them because their root is a
// `mod.rs`. A crate root has no such subtree, so all four are spelled out.
#[path = "kani_mutation_evidence_tests/compile_guard.rs"]
mod compile_guard;
#[path = "kani_mutation_evidence_tests/contracts.rs"]
mod contracts;
#[path = "kani_mutation_evidence_tests/discovery.rs"]
mod discovery;
#[path = "kani_mutation_evidence_tests/sandbox.rs"]
mod sandbox;
