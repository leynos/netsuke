//! Property suites for Ninja emission determinism.
//!
//! The `property_tests` segment is load-bearing rather than stylistic: the
//! repository's `make proptest` target selects suites by a module-path filter
//! matching `proptests` or `property_tests` segments, so a suite declared
//! outside one of those names is never run by that target at all.
//!
//! `strategy` holds the generator's own soundness properties; the
//! obligation-bearing suites arrive with the milestones that state them.

#[cfg(test)]
mod strategy;
