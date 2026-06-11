//! Kani harnesses for manifest-to-IR safety properties.

use super::*;

#[kani::proof]
fn duplicate_output_always_rejected() {
    let output_seed = kani::any::<u8>();
    kani::assume(output_seed < 2);

    let expected_output = symbolic_output_name(output_seed);
    let duplicates = vec![expected_output.to_owned()];

    match duplicate_output_error_from_paths(duplicates) {
        IrGenError::DuplicateOutput { outputs, .. } => {
            kani::assert(outputs.len() == 1, "one duplicate output is reported");
            kani::assert(
                output_matches(outputs.first(), expected_output),
                "reported duplicate must include the shared path",
            );
        }
        _ => kani::assert(false, "expected DuplicateOutput"),
    }
}

fn symbolic_output_name(seed: u8) -> &'static str {
    if seed == 0 { "out-a" } else { "out-b" }
}

fn output_matches(actual: Option<&String>, expected: &str) -> bool {
    actual.is_some_and(|output| output == expected)
}
