//! Property suites for Ninja emission determinism.
//!
//! The `property_tests` segment is load-bearing rather than stylistic: the
//! repository's `make proptest` target selects suites by a module-path filter
//! matching `proptests` or `property_tests` segments, so a suite declared
//! outside one of those names is never run by that target at all.
//!
//! `strategy` holds the generator's own soundness properties. `path_key`,
//! `no_loss`, `order`, and `defaults` hold the obligation-bearing suites, one
//! module per obligation: the first two draw graphs from the generator, while
//! `order` and `defaults` drive the ordering rules both emission paths share.
//! `support` holds the pieces the graph-drawing suites share — so that they
//! cannot drift into slightly different domains, which is a defect no gate
//! would catch: both would pass, and neither would be evidence about the
//! other.

#[cfg(test)]
mod defaults;
#[cfg(test)]
mod no_loss;
#[cfg(test)]
mod order;
#[cfg(test)]
mod path_key;
#[cfg(test)]
mod strategy;
#[cfg(test)]
mod support;
