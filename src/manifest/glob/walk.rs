//! Filesystem traversal helpers for glob expansion.
use super::{GlobEntryResult, GlobErrorContext, GlobErrorType, GlobPattern, create_glob_error};
use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs::Dir};
use minijinja::Error;

pub(super) fn open_root_dir(pattern: &GlobPattern) -> std::io::Result<Dir> {
    let candidate = normalized_or_raw(pattern);
    let path = Utf8Path::new(candidate);
    if path.is_absolute() {
        Dir::open_ambient_dir("/", ambient_authority())
    } else {
        Dir::open_ambient_dir(".", ambient_authority())
    }
}

pub(super) fn process_glob_entry(
    entry: GlobEntryResult,
    pattern: GlobPattern,
    root: &Dir,
) -> std::result::Result<Option<String>, Error> {
    match entry {
        Ok(path) => {
            let utf_path = Utf8PathBuf::try_from(path).map_err(|_| {
                create_glob_error(
                    &GlobErrorContext {
                        pattern: pattern.raw.clone(),
                        error_char: char::from(0),
                        position: pattern.raw.len(),
                        error_type: GlobErrorType::IoError,
                    },
                    Some("glob matched a non-UTF-8 path".to_owned()),
                )
            })?;
            let metadata = fetch_metadata(root, &utf_path).map_err(|err| {
                create_glob_error(
                    &GlobErrorContext {
                        pattern: pattern.raw.clone(),
                        error_char: char::from(0),
                        position: pattern.raw.len(),
                        error_type: GlobErrorType::IoError,
                    },
                    Some(err.to_string()),
                )
            })?;
            if !metadata.is_file() {
                return Ok(None);
            }
            Ok(Some(utf_path.as_str().replace(char::from(0x5c), "/")))
        }
        Err(e) => Err(create_glob_error(
            &GlobErrorContext {
                pattern: pattern.raw,
                error_char: char::from(0),
                position: 0,
                error_type: GlobErrorType::IoError,
            },
            Some(e.to_string()),
        )),
    }
}

fn fetch_metadata(root: &Dir, path: &Utf8Path) -> std::io::Result<cap_std::fs::Metadata> {
    if path.is_absolute() {
        let stripped = path
            .as_str()
            .trim_start_matches(|c| c == char::from(0x2f) || c == char::from(0x5c));
        if stripped.is_empty() {
            root.metadata(Utf8Path::new("."))
        } else {
            root.metadata(stripped)
        }
    } else {
        root.metadata(path)
    }
}

fn normalized_or_raw(p: &GlobPattern) -> &str {
    p.normalized
        .as_deref()
        .unwrap_or_else(|| panic!("normalised pattern must be present after validation"))
}
