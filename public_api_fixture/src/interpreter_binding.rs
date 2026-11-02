//! Type-check the public trusted-interpreter binding surface.

use netsuke::ir::BuildGraph;
use netsuke::ninja_gen::{
    GeneratedNinja, InterpreterExecutable, InterpreterExecutableError, NinjaGenError, RecipeShell,
    generate_bundle_with_interpreter, generate_with_interpreter,
};

/// Construct an interpreter binding as an external embedder.
///
/// # Errors
/// Return the constructor's typed rejection for a relative or NUL-bearing
/// path.
pub fn construct(path: &str) -> Result<InterpreterExecutable, InterpreterExecutableError> {
    InterpreterExecutable::from_absolute_path(path.into())
}

/// Match every public constructor rejection.
#[must_use]
pub const fn rejection_kind(error: &InterpreterExecutableError) -> &'static str {
    match error {
        InterpreterExecutableError::RelativePath { path } => {
            let _ = path;
            "relative"
        }
        InterpreterExecutableError::NulCharacter => "nul",
    }
}

/// Render Ninja text for an explicit shell and trusted executable.
///
/// # Errors
/// Return the backend's typed generation error.
pub fn render_text(
    graph: &BuildGraph,
    shell: RecipeShell,
    interpreter: &InterpreterExecutable,
) -> Result<String, NinjaGenError> {
    generate_with_interpreter(graph, shell, interpreter)
}

/// Render a Ninja bundle for an explicit shell and trusted executable.
///
/// # Errors
/// Return the backend's typed generation error.
pub fn render_bundle(
    graph: &BuildGraph,
    shell: RecipeShell,
    interpreter: &InterpreterExecutable,
) -> Result<GeneratedNinja, NinjaGenError> {
    generate_bundle_with_interpreter(graph, shell, interpreter)
}

/// Report which shell could not be rendered without a binding.
#[must_use]
pub const fn unbound_shell(error: &NinjaGenError) -> Option<RecipeShell> {
    match error {
        NinjaGenError::InterpreterBindingRequired { shell } => Some(*shell),
        _ => None,
    }
}
