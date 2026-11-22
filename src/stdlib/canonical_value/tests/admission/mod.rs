//! Shared fixtures and generated cases for bounded canonical admission.
//!
//! The two children split by what they supply. `support` holds the fixtures
//! every admission battery builds on: an explicit-ceiling budget, the
//! resource-refusal assertion, and the shared and independently expanded host
//! graphs. `properties` holds the generators and laws that range over shapes no
//! fixture names.
//!
//! They live together because they name one feature. The repository's module
//! layout contract groups siblings that share a name prefix into a directory
//! module rather than allowing a flat `admission_*` pair, and the prefix here
//! is not coincidental: both children exist for admission alone.

pub(super) mod properties;
pub(super) mod support;
