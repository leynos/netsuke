//! Path resolution helpers for the Ninja runner.
//! Canonicalises UTF-8 paths via capability-based handles.

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs as cap_fs};
use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

pub fn canonicalize_utf8_path(path: &Path) -> io::Result<Utf8PathBuf> {
    let utf8 = Utf8Path::from_path(path).ok_or_else(|| {
        io::Error::new(
            ErrorKind::InvalidData,
            format!("path {} is not valid UTF-8", path.display()),
        )
    })?;

    if utf8.as_str().is_empty() || utf8 == Utf8Path::new(".") {
        return canonicalize_current_dir();
    }

    if utf8.parent().is_none() && utf8.file_name().is_none() {
        return Ok(canonicalize_root_path(utf8));
    }

    if utf8.is_relative() {
        return canonicalize_relative_path(utf8);
    }

    canonicalize_absolute_path(utf8)
}

fn canonicalize_current_dir() -> io::Result<Utf8PathBuf> {
    let dir = cap_fs::Dir::open_ambient_dir(".", ambient_authority())?;
    let resolved = dir.canonicalize(Path::new("."))?;
    convert_path_to_utf8(resolved, Utf8Path::new("."))
}

fn canonicalize_root_path(utf8: &Utf8Path) -> Utf8PathBuf {
    utf8.to_path_buf()
}

fn canonicalize_relative_path(utf8: &Utf8Path) -> io::Result<Utf8PathBuf> {
    let dir = cap_fs::Dir::open_ambient_dir(".", ambient_authority())?;
    let resolved = dir.canonicalize(utf8.as_std_path())?;
    convert_path_to_utf8(resolved, utf8)
}

fn canonicalize_absolute_path(utf8: &Utf8Path) -> io::Result<Utf8PathBuf> {
    let parent = utf8.parent().unwrap_or_else(|| Utf8Path::new("/"));
    let handle = cap_fs::Dir::open_ambient_dir(parent.as_std_path(), ambient_authority())?;
    let relative = utf8.strip_prefix(parent).unwrap_or(utf8);
    let resolved = handle.canonicalize(relative.as_std_path())?;
    let canonical = convert_path_to_utf8(resolved, relative)?;
    if canonical.is_absolute() {
        Ok(canonical)
    } else {
        let mut absolute = parent.to_path_buf();
        absolute.push(&canonical);
        Ok(absolute)
    }
}

fn convert_path_to_utf8(buf: PathBuf, reference: &Utf8Path) -> io::Result<Utf8PathBuf> {
    Utf8PathBuf::from_path_buf(buf).map_err(|_| {
        io::Error::new(
            ErrorKind::InvalidData,
            format!("canonical path for {reference} is not valid UTF-8"),
        )
    })
}
