//! Compile-pass snippet for the accepted `cfg(kani)` name.

#![deny(unexpected_cfgs)]

#[cfg(kani)]
fn kani_only_item() {}

fn main() {
    #[cfg(kani)]
    kani_only_item();
}
