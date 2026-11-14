//! Verus proof for the shared resource-bound kernels.
//!
//! The modules here prove the arithmetic kernels in
//! `src/stdlib/resource_bounds/kernel.rs`: the `product` tuple count, the
//! binomial and falling-factorial cardinalities used by the `range` and
//! `permutations` helpers, and the running-total admission shared by the
//! streaming helpers. Each kernel is transcribed into an executable function
//! with a postcondition that states the exact value it returns or the exact
//! overflow it refuses.
//!
//! # What the proof covers, and what it does not
//!
//! The proof covers a transcription of the production kernel, not the compiled
//! kernel itself. Three library operations the kernel uses have no `vstd`
//! specification, and each is transcribed in a form whose meaning is fixed by
//! the surrounding Rust semantics:
//!
//! - `u64::checked_pow` becomes a fold of `checked_mul`, which has a `vstd`
//!   spec. The fold is the documented contract of `checked_pow`: repeated
//!   multiplication that returns `None` on the first overflow.
//! - `u64::min` becomes an explicit `if a < b { a } else { b }`.
//! - `Option::filter` becomes an explicit `match` on the retained predicate.
//!
//! Those gaps are narrowed outside Verus: the production kernels are also
//! checked by the Kani harnesses in
//! `src/stdlib/resource_bounds/verification.rs`, which run the compiled code
//! within stated bounds, and by Proptest against independent oracles.
//!
//! The trusted base is `vstd` alone. The proof contains no `assume`, `admit`,
//! or `external_body`, and no axiom beyond Verus's own arithmetic lemmas.
//!
//! # Running the proof
//!
//! ```sh
//! make verus VERUS_FLAGS='--proof-file verus/resource_bounds/main.rs'
//! ```

use vstd::prelude::*;

verus! {

mod model;
mod lens_product;
mod power_product;
mod binomial;
mod falling;
mod admit;

/// Provide the entry point Verus requires; it carries no proof of its own.
fn main() {}

}
