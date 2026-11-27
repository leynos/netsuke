//! Generated tests for manifest `foreach` and `when` expansion.
//! `strategy` models an authored entry, `model` predicts expansion output,
//! `support` drives the real entry point, and `coverage` keeps generators
//! non-vacuous. `make proptest` selects this suite by its `property_tests`
//! segment; the remaining modules state one obligation each.

mod coverage;
mod expansion_report;
mod foreach_sequences;
mod inject;
mod model;
mod order;
mod preserve;
mod shape_cases;
mod static_when;
mod strategy;
mod support;
mod when_removed;
