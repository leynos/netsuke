//! Tests for the default interpreter binding the library entry points resolve.
//!
//! The deterministic boundary lives in [`tests`], and the rendered form of an
//! arbitrary absolute interpreter path is covered by the properties in
//! [`properties`]. Both are test-only, so the parent module registers this
//! module under `#[cfg(test)]`.

mod properties;
mod tests;
