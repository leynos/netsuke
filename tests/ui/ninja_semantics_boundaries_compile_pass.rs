//! Control fixture proving the harness compiles the generated-Ninja boundary.
//!
//! Were the `--extern` wiring broken, the compile-fail fixtures would be
//! rejected because `test_support` could not be found rather than because the
//! text domains were swapped, and those tests would pass vacuously; this
//! fixture fails instead, naming the harness as the fault.

use test_support::ninja_semantics::{GeneratedNinja, RecipeNeedle};

fn main() {
    let document = GeneratedNinja::new("  command = echo hi\n");
    let needle = RecipeNeedle::new("echo hi");
    let _ = document.recipe_contains(needle);
}
