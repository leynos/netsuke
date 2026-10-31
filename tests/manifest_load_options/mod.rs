//! Module tree for the manifest loader entry-point characterization suite.
//!
//! `wrapper_equivalence` records one canonical-result snapshot per public entry
//! point over the fixtures applicable to it. `fidelity_property` asserts the
//! Full/Test differential property over generated pure manifests, and
//! `corpus_fidelity` asserts the same claim — equal, or a named refusal — over
//! every manifest the repository already keeps.
//!
//! `fixtures` holds the shared machinery — the corpus and its hazards, the
//! staging that gives a fixture a workspace, the snapshot settings, and the
//! canonical-result renderer — so the three suites describe *what* they compare
//! without restating *how*.
//!
//! The crate-internal query rows are *not* here: they live in
//! `src/manifest/tests/wrapper_equivalence.rs`, because the entry point they
//! characterize is crate-private and this integration crate cannot reach it.
//! They restate the few fixtures they need rather than sharing these.
mod corpus_fidelity;
mod fidelity_property;
mod fixtures;
mod wrapper_equivalence;
