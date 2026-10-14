//! Shared machinery for the manifest loader entry-point characterization suite.
//!
//! Four concerns live here, kept apart from the suites that use them:
//!
//! - [`fixture`] enumerates the characterization corpus and reports what each
//!   manifest forces on the loader that reads it — an injected reader, a fixed
//!   clock, or a refusing surface.
//! - [`corpus`] stages a fixture in a temporary workspace, copying exactly the
//!   siblings that fixture declared.
//! - [`settings`] builds the insta settings every row binds, pinned to one
//!   snapshot directory and to a temporary-path filter.
//! - [`canonical`] reduces a load outcome to the text a snapshot records, so a
//!   successful load and a failed one are snapshotted the same way.
//!
//! Every fixture is a file, which is what lets a path row pin the error text
//! that names it and gives the temporary-path filter something to replace.

pub mod canonical;
mod corpus;
mod fixture;
mod settings;

pub use self::{
    canonical::report,
    corpus::StagedFixture,
    fixture::{
        Capabilities, Case, cases, environment, exhausted_budget, fixed_clock_config,
        one_var_reader, path_entry_point, process_environment,
    },
    settings::settings,
};
