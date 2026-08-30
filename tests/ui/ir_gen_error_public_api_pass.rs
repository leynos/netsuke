//! Compile-pass fixture for the public IR-generation error surface.

use netsuke::ir::IrGenError;

/// Match the manifest-validation error exposed to external callers.
fn is_invalid_manifest(error: IrGenError) -> bool {
    matches!(error, IrGenError::InvalidManifest { message } if message == "missing recipe")
}

fn main() {
    assert!(is_invalid_manifest(IrGenError::InvalidManifest {
        message: "missing recipe",
    }));
}
