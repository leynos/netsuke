//! Compile the production build-script slice as part of Cargo's normal graph.
//!
//! The supported API must compile before the negative example is meaningful:
//!
//! ```
//! use netsuke_build_slice_fixture::cli::Cli;
//! let _ = <Cli as clap::CommandFactory>::command;
//! ```
//!
//! Runtime configuration discovery must remain outside this slice:
//!
//! ```compile_fail,E0432
//! use netsuke_build_slice_fixture::cli::discovery;
//! ```

// Compile production modules only in the ordinary library build, as build.rs
// does. Unit-test configuration would pull in their unrelated runtime tests;
// rustdoc exercises the two boundary examples against the compiled library.
#[cfg(not(any(test, doctest)))]
include!("slice.rs");
