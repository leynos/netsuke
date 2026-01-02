//! Shared helpers for CLI tests.

use std::ffi::OsString;

pub(super) fn os_args(args: &[&str]) -> Vec<OsString> {
    args.iter().map(|arg| OsString::from(*arg)).collect()
}
