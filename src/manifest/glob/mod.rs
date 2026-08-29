//! Filesystem glob expansion for manifest templates.
//!
//! [`glob_paths`] is the module's only boundary: `manifest` re-exports it as
//! the `glob()` Jinja helper and nothing else here is reachable from the crate
//! root. It takes a raw pattern, expands it, and returns the matching file
//! paths in the order the `glob` crate yields them, with directories filtered
//! out.
//!
//! The work is split across five private submodules:
//!
//! - `validate` rejects unbalanced braces before any filesystem access.
//! - `base` canonicalizes an injected base into a glob-compatible path.
//! - `normalize` maps separators onto the platform's and, on Unix, rewrites
//!   backslash escapes into the bracket classes the `glob` crate understands.
//!   [`GlobPattern`] pairs the caller's text with that normalized form.
//! - `walk` owns the filesystem side: it computes the pattern's literal
//!   directory prefix, opens a capability-scoped `cap_std` handle there, and
//!   runs the metadata check that filters each match.
//! - `diagnostics` records the bounded data the pure expansion query returns
//!   at the manifest orchestration boundary.
//! - The manifest adapter exposes only paths that are portable unquoted shell
//!   words. The public [`glob_paths`] query remains a filesystem API and does
//!   not impose that template-specific command-safety policy.
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

use base::resolve_relative_glob_base;
use camino::{Utf8Path, Utf8PathBuf};
use errors::{GlobErrorContext, GlobErrorType, create_glob_error};
use normalize::normalize_separators;
use validate::validate_brace_matching;
use walk::{open_root_dir, process_glob_entry};

#[cfg(unix)]
use normalize::force_literal_escapes;

#[derive(Debug, Clone)]
/// A glob pattern and its normalized representation.
struct GlobPattern {
    /// Pattern text as provided by the caller.
    raw: String,
    /// Platform-normalized pattern used for globbing.
    normalized: String,
}

impl GlobPattern {
    /// Access the pattern text as provided by the caller.
    #[must_use]
    #[expect(
        clippy::missing_const_for_fn,
        reason = "const String::as_str() not available on all MSRV targets"
    )]
    fn raw(&self) -> &str {
        self.raw.as_str()
    }

    /// Access the platform-normalized pattern suitable for globbing.
    #[must_use]
    #[expect(
        clippy::missing_const_for_fn,
        reason = "const String::as_str() not available on all MSRV targets"
    )]
    fn normalized(&self) -> &str {
        self.normalized.as_str()
    }

    /// Validate and normalize a glob pattern, preventing inconsistent state.
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

/// A completed glob expansion and the bounded outcomes it observed.
pub(super) struct GlobExpansion {
    /// Matched file paths in walker order.
    paths: Vec<String>,
    /// How the expansion terminated.
    outcome: GlobOutcome,
    /// Bounded observations of entries omitted from the paths.
    skipped: GlobSkippedEntries,
}

/// Terminal outcome of a glob expansion.
enum GlobOutcome {
    /// The pattern was expanded to completion.
    Matched,
    /// The pattern's literal directory prefix could not be opened.
    UnopenablePrefix,
}

/// Maximum unreachable-symlink paths retained for tracing one expansion.
const MAX_UNREACHABLE_SYMLINK_SAMPLES: usize = 4;

/// Bounded diagnostic data about entries omitted from an expansion.
#[derive(Default)]
struct GlobSkippedEntries {
    /// Number of matches the capability could not resolve through symlinks.
    unreachable_symlinks: usize,
    /// Short bounded sample of skipped symlink paths, retained for tracing.
    unreachable_symlink_samples: Vec<camino::Utf8PathBuf>,
    /// Number of matches that do not name a regular file.
    not_a_file: usize,
}

impl GlobSkippedEntries {
    /// Record an unreachable symlink while retaining a bounded trace sample.
    fn record_unreachable_symlink(&mut self, relative: camino::Utf8PathBuf) {
        self.unreachable_symlinks += 1;
        if self.unreachable_symlink_samples.len() < MAX_UNREACHABLE_SYMLINK_SAMPLES {
            self.unreachable_symlink_samples.push(relative);
        }
    }

    /// Record an entry that does not name a regular file.
    const fn record_not_a_file(&mut self) {
        self.not_a_file += 1;
    }
}

/// Entry selected by the capability-scoped metadata query.
#[derive(Debug)]
pub(super) enum GlobEntry {
    /// A matched regular file path retained until final result formatting.
    Path(Utf8PathBuf),
    /// A symlink the capability cannot resolve, given relative to the prefix.
    UnreachableSymlink(camino::Utf8PathBuf),
    /// The match does not name a regular file.
    NotAFile,
}

impl GlobExpansion {
    /// Consume the expansion and return the paths the query selected.
    pub(super) fn into_paths(self) -> Vec<String> {
        self.paths
    }

    /// Consume paths for use as unquoted manifest-template values.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// let expansion = GlobExpansion {
    ///     paths: vec!["src/main.c".to_owned()],
    ///     outcome: GlobOutcome::Matched,
    ///     skipped: GlobSkippedEntries::default(),
    /// };
    /// let paths = expansion.into_template_paths("src/*.c")?;
    /// assert_eq!(paths, vec!["src/main.c"]);
    /// # Ok::<(), minijinja::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error if a path contains characters that a command shell
    /// could reinterpret rather than preserving as one literal word.
    pub(super) fn into_template_paths(
        self,
        pattern: &str,
    ) -> std::result::Result<Vec<String>, Error> {
        if self.paths.iter().all(|path| is_shell_inert_path(path)) {
            return Ok(self.paths);
        }
        diagnostics::record_template_path_rejection();
        Err(create_glob_error(
            &GlobErrorContext {
                pattern: pattern.to_owned(),
                error_char: char::from(0),
                position: pattern.len(),
                error_type: GlobErrorType::IoError,
            },
            Some("glob matched a path containing characters that require shell quoting".to_owned()),
        ))
    }
}

/// Return whether `path` is portable as one unquoted shell word.
///
/// This predicate belongs only to the Jinja `glob()` adapter. Graph paths and
/// direct [`glob_paths`] callers have separate escaping boundaries and must not
/// reuse this deliberately restrictive policy.
fn is_shell_inert_path(path: &str) -> bool {
    !path.is_empty()
        && path.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b',' | b'.' | b'/' | b'_' | b'-' | b':')
        })
}
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
/// let _: fn(&str, Option<&camino::Utf8Path>) -> _ = glob_paths;
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
pub fn glob_paths(
    pattern: &str,
    base: Option<&Utf8Path>,
) -> std::result::Result<Vec<String>, Error> {
    expand_glob(pattern, base).map(GlobExpansion::into_paths)
}

/// Expand a pattern and return its bounded diagnostic data without recording it.
///
/// `base` anchors relative patterns: when supplied and the pattern is not
/// absolute, the pattern is joined onto `base` before matching and the base is
/// stripped from the returned paths, so results keep their pattern-relative
/// spelling. `None` falls back to the process current directory, the
/// composition-root behaviour retained for string parsing.
pub(super) fn expand_glob(
    pattern: &str,
    base: Option<&Utf8Path>,
) -> std::result::Result<GlobExpansion, Error> {
    use glob::{MatchOptions, glob_with};

    let opts = MatchOptions {
        case_sensitive: true,
        require_literal_separator: true,
        require_literal_leading_dot: false,
    };

    let prepared = PreparedGlob::new(pattern, base)?;
    let entries = glob_with(&prepared.search, opts).map_err(|e| {
        create_glob_error(
            &GlobErrorContext {
                pattern: prepared.pattern.raw().to_owned(),
                error_char: char::from(0),
                position: 0,
                error_type: GlobErrorType::InvalidPattern,
            },
            Some(e.to_string()),
        )
    })?;

    // `search` already embeds the injected base (`base.join(normalized)`), so
    // the capability root must be opened from the literal prefix as written
    // rather than passing `base` again: doing the latter would reopen the base
    // directory and then traverse its own name, doubling the path component.
    let Some(root) = open_root_dir(&prepared.search, None).map_err(|e| {
        create_glob_error(
            &GlobErrorContext {
                pattern: prepared.pattern.raw().to_owned(),
                error_char: char::from(0),
                position: 0,
                error_type: GlobErrorType::IoError,
            },
            Some(e.to_string()),
        )
    })?
    else {
        // The pattern's literal directory prefix does not exist, so the
        // pattern cannot match anything.
        return Ok(GlobExpansion {
            outcome: GlobOutcome::UnopenablePrefix,
            paths: Vec::new(),
            skipped: GlobSkippedEntries::default(),
        });
    };

    let mut paths = Vec::new();
    let mut skipped = GlobSkippedEntries::default();
    for entry in entries {
        match process_glob_entry(entry, &prepared.pattern, &root)? {
            GlobEntry::Path(path) => paths.push(strip_base(prepared.strip.as_deref(), &path)),
            GlobEntry::UnreachableSymlink(relative) => {
                skipped.record_unreachable_symlink(relative);
            }
            GlobEntry::NotAFile => skipped.record_not_a_file(),
        }
    }
    Ok(GlobExpansion {
        paths,
        outcome: GlobOutcome::Matched,
        skipped,
    })
}

/// A validated glob pattern, its search text, and the base to strip.
///
/// Preparation is pure text work: the pattern is validated and normalised
/// once, the injected base is resolved to a symlink-free absolute path, and
/// the resolved base is embedded in the search text only when the pattern is
/// relative. Keeping it separate from the filesystem walk means [`expand_glob`]
/// only orchestrates matching, prefix opening, and result collection.
struct PreparedGlob {
    /// Validated pattern and its normalised spelling.
    pattern: GlobPattern,
    /// Search text handed to `glob_with`: the normalised pattern, prefixed
    /// with the resolved base when the pattern is relative.
    search: String,
    /// Canonicalized base stripped from matches, present exactly when relative.
    strip: Option<Utf8PathBuf>,
}

impl PreparedGlob {
    /// Prepare `pattern` for matching against the optional injected `base`.
    ///
    /// # Errors
    ///
    /// Returns an error when the pattern fails brace validation or when the
    /// injected base cannot be canonicalized.
    fn new(pattern: &str, base: Option<&Utf8Path>) -> std::result::Result<Self, Error> {
        let pattern_state = GlobPattern::new(pattern)?;
        let normalized = pattern_state.normalized();
        let resolved_base = match base {
            // Only a relative pattern is anchored to the injected base; an
            // absolute pattern names its own location and must ignore `base`.
            Some(dir) if !Utf8Path::new(normalized).is_absolute() => {
                Some(resolve_relative_glob_base(dir)?)
            }
            _ => None,
        };
        let (search, strip) = resolved_base.as_deref().map_or_else(
            || (normalized.to_owned(), None),
            |dir| {
                // Escape the base as a literal: workspace directories may
                // legitimately contain glob metacharacters (`*`, `?`, `[`,
                // `{`), which would otherwise be compiled as wildcards and
                // match decoy directories instead of the manifest's base.
                // The user's pattern keeps its syntax.
                let escaped = glob::Pattern::escape(dir.as_str());
                let separator = std::path::MAIN_SEPARATOR;
                (
                    // Match the separator used by `GlobPattern::new` and
                    // `walk::literal_dir_prefix`; mixing slash styles breaks
                    // literal-prefix discovery on Windows.
                    format!("{escaped}{separator}{normalized}"),
                    Some(dir.to_path_buf()),
                )
            },
        );
        Ok(Self {
            pattern: pattern_state,
            search,
            strip,
        })
    }
}
/// Remove an injected base directory from a matched path, restoring the
/// pattern-relative spelling the caller supplied.
///
/// Only relative patterns with an injected base are affected: `strip` is
/// `Some` exactly when the matcher ran against `base.join(pattern)`, so every
/// match starts with `base` and the lexical strip cannot fail in a way that
/// would drop real matches. The fallback returns the path unchanged.
fn strip_base(base: Option<&Utf8Path>, path: &Utf8Path) -> String {
    let relative = base
        .and_then(|dir| path.strip_prefix(dir).ok())
        .unwrap_or(path);
    // Format once, after any lexical base stripping, so a matched path does
    // not first allocate a normalized String only to allocate again to rebase.
    relative.as_str().replace('\\', "/")
}
/// Record the bounded observations from a completed expansion.
///
/// `glob_paths` deliberately does not call this function: it is a pure query.
/// The manifest-template adapter records observations after it calls
/// [`expand_glob`].
pub(super) fn record_expansion(expansion: &GlobExpansion) {
    diagnostics::record(expansion);
}

#[cfg(test)]
mod tests;

mod base;
