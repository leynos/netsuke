//! Generated tests for manifest `foreach` and `when` expansion.
//!
//! The module path carries the `property_tests` segment because `make
//! proptest` selects suites by that segment; a suite placed beside the
//! example cases never runs there. The suite began as the relocation of the
//! two generated modules that used to live under `tests/`: `expansion_report`
//! covers entry counts, index sequencing, filtering metadata, and determinism,
//! and `foreach_sequences` covers sequence values, the cardinality budget, and
//! source key order. The obligations this suite exists to discharge are stated
//! in `docs/netsuke-design.md` section 2.5 and in the 4.3.2 ExecPlan.

mod expansion_report;
mod foreach_sequences;
