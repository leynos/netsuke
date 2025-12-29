//! Generic parse-and-store helpers for BDD step definitions.
//!
//! Provides a unified pattern for parsing operations that store results
//! or errors in `TestWorld` slots, eliminating duplication across domain
//! step modules (cli.rs, manifest.rs, ir.rs).
//!
//! # Example
//!
//! ```ignore
//! use crate::bdd::helpers::parse_store::store_parse_outcome;
//!
//! fn parse_manifest_inner(world: &TestWorld, path: &ManifestPath) {
//!     let outcome = manifest::from_path(path.as_str())
//!         .map_err(|e| display_error_chain(e.as_ref()));
//!     store_parse_outcome(&world.manifest, &world.manifest_error, outcome);
//! }
//! ```

use rstest_bdd::Slot;
use std::cell::RefCell;

use crate::bdd::fixtures::RefCellOptionExt;

/// Store a parse outcome in the appropriate world slots.
///
/// For `Ok(value)`:
/// - Sets the value slot with the parsed value
/// - Clears the error slot
///
/// For `Err(error)`:
/// - Clears the value slot
/// - Sets the error slot with the error string
///
/// This pattern is used consistently across CLI, manifest, and IR parsing
/// to ensure uniform handling of success and failure cases.
pub fn store_parse_outcome<T>(
    value_slot: &RefCell<Option<T>>,
    error_slot: &Slot<String>,
    outcome: Result<T, String>,
) {
    match outcome {
        Ok(value) => {
            value_slot.set_value(value);
            error_slot.clear();
        }
        Err(error) => {
            value_slot.clear_value();
            error_slot.set(error);
        }
    }
}
