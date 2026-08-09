//! Filesystem glob expansion for manifest templates.
//!
//! [`glob_paths`] is the module's only boundary: `manifest` re-exports it as
//! the `glob()` Jinja helper and nothing else here is reachable from the crate
//! root. It takes a raw pattern, expands it, and returns the matching file
//! paths in the order the `glob` crate yields them, with directories filtered
//! out.
//!
//! The work is split across four private submodules:
//!
//! - `validate` rejects unbalanced braces before any filesystem access.
//! - `normalize` maps separators onto the platform's and, on Unix, rewrites
//!   backslash escapes into the bracket classes the `glob` crate understands.
//!   [`GlobPattern`] pairs the caller's text with that normalised form.
//! - `walk` owns the filesystem side: it computes the pattern's literal
//!   directory prefix, opens a capability-scoped `cap_std` handle there, and
//!   runs the metadata check that filters each match.
//! - `diagnostics` records the two outcomes that are expected rather than
//!   erroneous — an unopenable prefix and a match no capability can reach.
//!
//! Matching itself belongs to the `glob` crate, which traverses the filesystem
//! ambiently; only the metadata check is capability-scoped. `walk`'s module
//! documentation and
//! [ADR-010](https://github.com/leynos/netsuke/blob/main/docs/adr-010-scope-glob-capability-to-literal-prefix.md)
//! describe that boundary and why it remains.
use minijinja::Error;

mod diagnostics;
mod errors;
mod normalize;
mod validate;
mod walk;

use errors::{GlobErrorContext, GlobErrorType, create_glob_error};
use normalize::normalize_separators;
use validate::validate_brace_matching;
use walk::{open_root_dir, process_glob_entry};

#[cfg(unix)]
use normalize::force_literal_escapes;

#[derive(Debug, Clone)]
/// A glob pattern and its normalised representation.
struct GlobPattern {
    raw: String,
    normalized: String,
}

impl GlobPattern {
    /// Access the pattern as provided by the caller.
    #[must_use]
    #[expect(
        clippy::missing_const_for_fn,
        reason = "const String::as_str() not available on all MSRV targets"
    )]
    fn raw(&self) -> &str {
        self.raw.as_str()
    }

    /// Access the platform-normalised pattern suitable for globbing.
    #[must_use]
    #[expect(
        clippy::missing_const_for_fn,
        reason = "const String::as_str() not available on all MSRV targets"
    )]
    fn normalized(&self) -> &str {
        self.normalized.as_str()
    }

    /// Validate and normalise a glob pattern, preventing inconsistent state.
    ///
    /// # Errors
    ///
    /// Returns an error when brace validation fails.
    fn new(raw: &str) -> std::result::Result<Self, Error> {
        validate_brace_matching(raw)?;

        #[cfg(unix)]
        let normalized = {
            let normalized = normalize_separators(raw);
            force_literal_escapes(&normalized)
        };
        #[cfg(not(unix))]
        let normalized = normalize_separators(raw);

        Ok(Self {
            raw: raw.to_owned(),
            normalized,
        })
    }
}

/// Result of iterating a single entry produced by the `glob` crate walker.
///
/// Internal to the glob module: only [`glob_paths`] and the `walk` submodule
/// consume it, so it is deliberately not part of the public API surface.
///
/// `#[deny(unreachable_pub)]` on the `mod glob;` declaration in
/// [`crate::manifest`] rejects `pub` items that are still unreachable from the
/// crate root, so widening this alias to `pub` fails the build. The doctests on
/// [`glob_paths`] pin the same boundary from a downstream crate's point of
/// view; they live there because that is where rustdoc will run them.
type GlobEntryResult = std::result::Result<std::path::PathBuf, glob::GlobError>;

/// Expand a glob pattern and collect the matching UTF-8 file paths.
///
/// This is the only public item in the glob module: `netsuke::manifest`
/// re-exports it and nothing else. The examples below hold that boundary from
/// a downstream crate's point of view, and are attached here rather than to the
/// private items they describe because rustdoc renders and runs the examples of
/// public items.
///
/// # Errors
///
/// Returns an error when the pattern is syntactically invalid, when
/// capability-restricted filesystem access fails, or when a match contains
/// non-UTF-8 data.
///
/// # Examples
///
/// The entry point is reachable through the re-export:
///
/// ```
/// use netsuke::manifest::glob_paths;
/// let _: fn(&str) -> _ = glob_paths;
/// ```
///
/// The module's internals are not. `GlobEntryResult` is a private alias inside
/// the private `manifest::glob` module, so no downstream path names it:
///
/// ```compile_fail,E0603
/// use netsuke::manifest::glob::GlobEntryResult;
/// ```
///
/// The passing example above is the control for this rejection: it fails
/// instead if the rustdoc harness wiring breaks, so the `compile_fail` block
/// cannot pass vacuously.
pub fn glob_paths(pattern: &str) -> std::result::Result<Vec<String>, Error> {
    use glob::{MatchOptions, glob_with};

    let opts = MatchOptions {
        case_sensitive: true,
        require_literal_separator: true,
        require_literal_leading_dot: false,
    };

    let pattern_state = GlobPattern::new(pattern)?;

    let Some(root) = open_root_dir(&pattern_state).map_err(|e| {
        create_glob_error(
            &GlobErrorContext {
                pattern: pattern_state.raw().to_owned(),
                error_char: char::from(0),
                position: 0,
                error_type: GlobErrorType::IoError,
            },
            Some(e.to_string()),
        )
    })?
    else {
        // The pattern's literal directory prefix does not exist, so the
        // pattern cannot match anything. `open_root_dir` has already recorded
        // the outcome.
        return Ok(Vec::new());
    };

    let entries = glob_with(pattern_state.normalized(), opts).map_err(|e| {
        create_glob_error(
            &GlobErrorContext {
                pattern: pattern_state.raw().to_owned(),
                error_char: char::from(0),
                position: 0,
                error_type: GlobErrorType::InvalidPattern,
            },
            Some(e.to_string()),
        )
    })?;
    let mut paths = Vec::new();
    for entry in entries {
        if let Some(p) = process_glob_entry(entry, &pattern_state, &root)? {
            paths.push(p);
        }
    }
    diagnostics::record_expansion_matched(&pattern_state, paths.len());
    Ok(paths)
}

#[cfg(test)]
mod tests;
