//! Definition-authority tests for automatically discovered occurrences.
//!
//! Root authority is decided once, from the provenance of the occurrence,
//! rather than inferred later from the flattened layer list. `cases` covers the
//! written classifier branches, including the fail-closed paths that keep an
//! unprovable occurrence from being treated as operator configuration.
//! `properties` states the same contract over generated arrangements, so a
//! written case cannot be the only evidence for a rule that must hold for every
//! ordering.

mod cases;
mod properties;
