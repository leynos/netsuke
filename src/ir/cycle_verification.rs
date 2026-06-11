//! Kani harnesses for IR cycle detection properties.

#[kani::proof]
#[kani::unwind(2)]
fn scaffold_smoke() {
    kani::assert(true, "scaffold: replace with real cycle-detection harness");
}
