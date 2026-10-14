//! The document being searched must not compile as the text to search for.
//!
//! `RecipeNeedle::new` wraps the text a recipe must carry, so accepting a
//! `GeneratedNinja` here would make every search look for the whole manifest
//! inside its own recipe semantics, and would let encoded payload text be
//! searched as if it were plaintext Ninja text.

use test_support::ninja_semantics::{GeneratedNinja, RecipeNeedle};

fn main() {
    let document = GeneratedNinja::new("  command = echo hi\n");
    let _ = RecipeNeedle::new(document);
}
