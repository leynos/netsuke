//! POSIX heredoc tests for command interpolation.
//!
//! Delimiter lowering, body tracking, and the property model that checks both
//! form one subject, so they live in one directory module rather than beside
//! each other under a shared file-name prefix. The helpers every case uses are
//! in [`support`]; the named cases and the generated cases stay apart only so
//! each file fits the workspace's module-size limit.

mod delimiter_tests;
mod expansion_tests;
mod property_tests;
mod support;
