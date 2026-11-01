//! Unit batteries for bounded materialization.
//!
//! Every case in this subtree drives the public surface — the ceilings,
//! `CardinalityRequest`, and its typed refusal — so a refusal is the value a
//! consumer receives and an admitted count is one the public API produced. The
//! kernel's arithmetic is reached only where a battery names it explicitly,
//! which is the `admission` battery's `classify` property and the cap
//! witnesses in `cardinality`.
//!
//! The batteries split by the kind of judgement each makes. `oracle` owns the
//! independent `u128` expectations that no production path computes;
//! `properties` owns the generated cases and their class-coverage tripwires;
//! `cardinality` owns the named arithmetic witnesses; `admission` owns the
//! boundary table and the argument that constructing a refusal allocates
//! nothing.

use super::*;

mod admission;
mod cardinality;
mod oracle;
mod properties;
