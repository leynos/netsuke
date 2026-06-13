//! Compile-fail snippet for a cfg name outside the Kani cfg contract.

#![deny(unexpected_cfgs)]

#[cfg(netsuke_unknown_cfg_for_ui_test)]
fn unexpected_cfg_item() {}

fn main() {}
