//! The text being searched for must not compile as the document to search.
//!
//! `GeneratedNinja::new` wraps a whole generated manifest and
//! `RecipeNeedle::new` wraps the text a recipe must carry. Accepting a needle
//! where a document belongs would let a call site decode a whole manifest as if
//! it were a Base64 payload, and would search the generated text for a needle
//! that is itself the generated text.

use test_support::ninja_semantics::{GeneratedNinja, RecipeNeedle};

fn main() {
    let needle = RecipeNeedle::new("echo hi");
    let _ = GeneratedNinja::new(needle);
}
