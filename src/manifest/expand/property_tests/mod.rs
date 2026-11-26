//! Generated tests for manifest `foreach` and `when` expansion.
//!
//! The module path carries the `property_tests` segment because `make
//! proptest` selects suites by that segment; a suite placed beside the
//! example cases never runs there. The suite began as the relocation of the
//! two generated modules that used to live under `tests/`: `expansion_report`
//! covers entry counts, index sequencing, filtering metadata, and determinism,
//! and `foreach_sequences` covers sequence values, the cardinality budget, and
//! source key order.
//!
//! The suites added for the 4.3.2 obligations share one generator and one
//! oracle. `strategy` builds an authored-entry model, `model` predicts what
//! expansion must produce from it, and `support` drives the real expansion
//! entry point. The properties then state one obligation each:
//!
//! - `preserve`: every non-control field is copied verbatim;
//! - `order`: the entry-level and `vars` key order is the authoring order;
//! - `when_removed`: no expanded entry carries a control key and the filtered
//!   counts are exact;
//! - `inject`: every generated entry carries its own `item` and `index`;
//! - `static_when`: a static entry survives exactly when its condition holds;
//! - `coverage`: each generator still reaches every class those properties
//!   depend on, with a negative control for the threshold check.

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
