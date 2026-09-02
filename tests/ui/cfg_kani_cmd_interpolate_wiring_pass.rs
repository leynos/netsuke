//! Compile-pass fixture for command-interpolation Kani declarations.

#![deny(unexpected_cfgs)]

const INS_TOKEN: &str = "__NETSUKE_INS_PLACEHOLDER__";
const OUTS_TOKEN: &str = "__NETSUKE_OUTS_PLACEHOLDER__";

const _: () = assert!(
    matches!(INS_TOKEN.as_bytes().first(), Some(b'_'))
        && matches!(OUTS_TOKEN.as_bytes().first(), Some(b'_')),
    "the marker fallback in find_substitution only runs at underscore positions",
);

#[cfg(kani)]
#[path = "cfg_kani_cmd_interpolate_verification.rs"]
mod verification;

fn main() {
    verification::declarations_compile();
}
