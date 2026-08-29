//! Escape injected glob-base paths without corrupting platform roots.
//!
//! This module owns only the private translation from a resolved filesystem
//! path to glob search text. [`super::PreparedGlob`] is its sole caller; it
//! supplies a canonical base, while this module preserves path syntax and
//! escapes only ordinary component names.

use camino::{Utf8Component, Utf8Path};

/// Escape normal path components for glob compilation without changing roots.
///
/// Windows canonicalization can yield an extended-length prefix such as
/// `\\?\C:`. The `?` in that prefix is path syntax rather than a glob token,
/// so escaping the complete path would invalidate it. Prefix and root
/// components therefore remain verbatim; only ordinary component names are
/// escaped.
pub(super) fn escape_glob_literal_path(path: &Utf8Path) -> String {
    let separator = std::path::MAIN_SEPARATOR;
    let mut escaped = String::new();
    let mut needs_separator = false;

    for component in path.components() {
        match component {
            Utf8Component::Prefix(prefix) => {
                escaped.push_str(prefix.as_str());
                needs_separator = false;
            }
            Utf8Component::RootDir => {
                escaped.push(separator);
                needs_separator = false;
            }
            Utf8Component::CurDir => {
                append_glob_path_component(&mut escaped, ".", separator, &mut needs_separator);
            }
            Utf8Component::ParentDir => {
                append_glob_path_component(&mut escaped, "..", separator, &mut needs_separator);
            }
            Utf8Component::Normal(name) => {
                let literal = glob::Pattern::escape(name);
                append_glob_path_component(&mut escaped, &literal, separator, &mut needs_separator);
            }
        }
    }
    escaped
}

/// Append one escaped component while preserving host-native path semantics.
fn append_glob_path_component(
    path: &mut String,
    component: &str,
    separator: char,
    needs_separator: &mut bool,
) {
    if *needs_separator {
        path.push(separator);
    }
    path.push_str(component);
    *needs_separator = true;
}

#[cfg(all(test, windows))]
mod tests {
    use super::escape_glob_literal_path;
    use camino::Utf8Path;

    /// Preserve a Windows extended-length prefix while escaping ordinary components.
    #[test]
    fn preserves_verbatim_prefix() {
        let path = Utf8Path::new(r"\\?\C:\work\literal[ab]base");
        assert_eq!(
            escape_glob_literal_path(path),
            r"\\?\C:\work\literal[[]ab[]]base"
        );
    }
}
