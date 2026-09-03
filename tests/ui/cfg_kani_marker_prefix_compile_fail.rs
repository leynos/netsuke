//! Compile-fail fixture for the command-interpolation marker-prefix invariant.

#![deny(unexpected_cfgs)]

const INS_TOKEN: &str = "$in";
const OUTS_TOKEN: &str = "__NETSUKE_OUTS_PLACEHOLDER__";

const _: () = assert!(
    matches!(INS_TOKEN.as_bytes().first(), Some(b'_'))
        && matches!(OUTS_TOKEN.as_bytes().first(), Some(b'_')),
    "the marker fallback in find_substitution only runs at underscore positions",
);

fn main() {}
