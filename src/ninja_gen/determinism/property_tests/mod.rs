//! Property suites for Ninja emission determinism.
//!
//! The `property_tests` segment is load-bearing rather than stylistic: the
//! repository's `make proptest` target selects suites by a module-path filter
//! matching `proptests` or `property_tests` segments, so a suite declared
//! outside one of those names is never run by that target at all.
//!
//! `strategy` holds the generator's own soundness properties. `path_key` and
//! `no_loss` hold the obligation-bearing suites that run over graphs drawn from
//! it, one module per obligation, and `support` holds the pieces they share —
//! so that the two suites cannot drift into slightly different domains, which
//! is a defect no gate would catch: both would pass, and neither would be
//! evidence about the other.

#[cfg(test)]
mod no_loss;
#[cfg(test)]
mod path_key;
#[cfg(test)]
mod strategy;
#[cfg(test)]
mod support;
