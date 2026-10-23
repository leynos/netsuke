//! Type-check the public IR-generation error surface.

use netsuke::ir::IrGenError;

/// Report whether `error` is the manifest-validation error for a missing recipe.
#[must_use]
pub fn is_missing_recipe(error: &IrGenError) -> bool {
    matches!(error, IrGenError::InvalidManifest { message } if *message == "missing recipe")
}

/// Construct the manifest-validation error as an external caller.
#[must_use]
pub const fn missing_recipe() -> IrGenError {
    IrGenError::InvalidManifest {
        message: "missing recipe",
    }
}
