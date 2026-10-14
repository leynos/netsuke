//! Soundness and non-vacuity properties for the shared graph generator.
//!
//! These are infrastructure checks, not determinism claims. A generator that
//! produced empty, malformed, or self-identical materializations would let
//! every later obligation pass without testing anything, and the failure mode
//! would be invisible: the property under test would report success. Each of
//! the checks below therefore targets one way the generator could be vacuous.
//!
//! A property belonging to an obligation (`OBL-ORDER`, `OBL-PATHKEY`, and the
//! rest) does not live here. Only `EP-M3`'s own non-vacuity checks do.

#[cfg(test)]
mod graph;
