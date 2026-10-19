//! Bounded graph generation for the determinism obligations.
//!
//! [`graph::GraphSpec`] is the shared artefact every property in this plan
//! draws from: it holds a graph's parts unresolved so that one specification
//! can be materialized under several insertion orders, which is what makes an
//! order-independence claim falsifiable rather than merely plausible.
//! [`classification::Classification`] supplies the per-case shape counts that
//! keep a generated counter-example diagnosable and a vacuous run visible.
//!
//! The generator is deliberately *not* built on `graph_view`'s
//! `arb_graph_inputs`. That strategy filters colliding edges away, which is the
//! vacuity defect this plan forbids, and its perturbation is a vector reversal
//! that a short input can leave unchanged. See the plan's `Decision log`.

pub(crate) mod classification;
pub(crate) mod graph;
pub(crate) mod manifest;

pub(crate) use classification::{ALWAYS_RESTAT_ALL, Classification};
