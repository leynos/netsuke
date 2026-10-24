//! Determinism obligations for Ninja emission.
//!
//! The emitter's contract is that the bytes it writes are a function of the
//! build graph's *contents*, never of the order a caller happened to insert
//! them in. Discharging that needs more than an example: it needs a generator
//! producing well-formed graphs whose insertion orders can be varied
//! independently of their contents.
//!
//! That generator and the properties stated over it live here, beside the
//! emitter they constrain, because a `#[cfg(test)]` module declared under
//! `ninja_gen` is reachable from every library-side property suite while
//! `test_support` is not.
//!
//! Both children are `#[cfg(test)]`, so none of this is compiled into a
//! release artefact.

#[cfg(test)]
pub(crate) mod property_tests;

#[cfg(test)]
pub(crate) mod strategy;
