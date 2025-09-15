//! File-type predicates for Jinja templates.
//!
//! Registers `dir`, `file`, and `symlink` tests on all platforms. On Unix it
//! also provides `pipe`, `block_device`, `char_device`, and the legacy `device`
//! test. On other platforms `pipe` and `device` are stubbed to always return
//! `false`.
//!
//! I/O errors yield [`ErrorKind::InvalidOperation`] while missing paths return
//! `Ok(false)` rather than an error.

use camino::Utf8Path;
#[cfg(unix)]
use cap_std::fs::FileTypeExt;
use cap_std::{ambient_authority, fs, fs_utf8::Dir};
use minijinja::{Environment, Error, ErrorKind, value::Value};
use std::io;

fn is_dir(ft: fs::FileType) -> bool {
    ft.is_dir()
}
fn is_file(ft: fs::FileType) -> bool {
    ft.is_file()
}
fn is_symlink(ft: fs::FileType) -> bool {
    ft.is_symlink()
}
#[cfg(unix)]
fn is_fifo(ft: fs::FileType) -> bool {
    ft.is_fifo()
}
#[cfg(unix)]
fn is_block_device(ft: fs::FileType) -> bool {
    ft.is_block_device()
}
#[cfg(unix)]
fn is_char_device(ft: fs::FileType) -> bool {
    ft.is_char_device()
}
#[cfg(unix)]
fn is_device(ft: fs::FileType) -> bool {
    is_block_device(ft) || is_char_device(ft)
}

type FileTest = (&'static str, fn(fs::FileType) -> bool);

/// Register standard library helpers with the Jinja environment.
///
/// # Examples
/// ```
/// use minijinja::{Environment, context};
/// use netsuke::stdlib;
///
/// let mut env = Environment::new();
/// stdlib::register(&mut env);
/// let tmpl = env
///     .compile("{% if path is dir %}yes{% endif %}")
///     .unwrap();
/// let rendered = tmpl.render(context!(path => ".")).unwrap();
/// assert_eq!(rendered, "yes");
/// ```
pub fn register(env: &mut Environment<'_>) {
    const TESTS: &[FileTest] = &[
        ("dir", is_dir),
        ("file", is_file),
        ("symlink", is_symlink),
        #[cfg(unix)]
        ("pipe", is_fifo),
        #[cfg(unix)]
        ("block_device", is_block_device),
        #[cfg(unix)]
        ("char_device", is_char_device),
        // Deprecated combined test; prefer block_device or char_device.
        #[cfg(unix)]
        ("device", is_device),
    ];

    for &(name, pred) in TESTS {
        env.add_test(name, move |val: Value| -> Result<bool, Error> {
            if let Some(s) = val.as_str() {
                return is_file_type(Utf8Path::new(s), pred);
            }
            Ok(false)
        });
    }

    #[cfg(not(unix))]
    {
        env.add_test("pipe", |_val: Value| Ok(false));
        env.add_test("block_device", |_val: Value| Ok(false));
        env.add_test("char_device", |_val: Value| Ok(false));
        env.add_test("device", |_val: Value| Ok(false));
    }
}

/// Determine whether `path` matches the given file type predicate.
///
/// Uses `Dir::symlink_metadata`, so symbolic links are inspected without
/// following the target. `is_symlink` and `is_file`/`is_dir` cannot both be
/// true for the same path.
///
/// Returns `Ok(false)` if the path does not exist.
fn is_file_type<F>(path: &Utf8Path, predicate: F) -> Result<bool, Error>
where
    F: Fn(fs::FileType) -> bool,
{
    let (dir_path, file_name) = match (path.parent(), path.file_name()) {
        (Some(parent), Some(name)) => (parent, name),
        (Some(parent), None) => (parent, "."),
        (None, Some(name)) => (Utf8Path::new("."), name),
        (None, None) => (Utf8Path::new("."), "."),
    };
    let dir = match Dir::open_ambient_dir(dir_path, ambient_authority()) {
        Ok(d) => d,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(err) => {
            return Err(Error::new(
                ErrorKind::InvalidOperation,
                format!("cannot open directory for {path}: {err}"),
            )
            .with_source(err));
        }
    };
    match dir.symlink_metadata(Utf8Path::new(file_name)) {
        Ok(md) => Ok(predicate(md.file_type())),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(Error::new(
            ErrorKind::InvalidOperation,
            format!("cannot read metadata for {path}: {err}"),
        )
        .with_source(err)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;
    use cap_std::fs_utf8::Dir;
    use rstest::{fixture, rstest};
    #[cfg(unix)]
    use rustix::fs::{Dev, FileType, Mode, mknodat};
    use tempfile::tempdir;

    #[fixture]
    fn file_paths() -> (
        tempfile::TempDir,
        Utf8PathBuf,
        Utf8PathBuf,
        Utf8PathBuf,
        Utf8PathBuf,
        Utf8PathBuf,
        Utf8PathBuf,
    ) {
        let temp = tempdir().expect("tempdir");
        let root = Utf8PathBuf::from_path_buf(temp.path().to_path_buf()).expect("utf8");
        let dir = root.join("d");
        let file = root.join("f");
        let link = root.join("s");
        let fifo = root.join("p");
        let handle = Dir::open_ambient_dir(&root, ambient_authority()).expect("ambient");
        handle.create_dir("d").expect("dir");
        handle.write("f", b"x").expect("file");
        handle.symlink("f", "s").expect("symlink");
        #[cfg(unix)]
        mknodat(
            &handle,
            "p",
            FileType::Fifo,
            Mode::RUSR | Mode::WUSR,
            Dev::default(),
        )
        .expect("fifo");
        let bdev = Utf8PathBuf::from("/dev/loop0");
        let cdev = Utf8PathBuf::from("/dev/null");
        (temp, dir, file, link, fifo, bdev, cdev)
    }

    #[rstest]
    fn detects_dir(
        file_paths: (
            tempfile::TempDir,
            Utf8PathBuf,
            Utf8PathBuf,
            Utf8PathBuf,
            Utf8PathBuf,
            Utf8PathBuf,
            Utf8PathBuf,
        ),
    ) {
        let (_, dir, _, _, _, _, _) = file_paths;
        assert!(is_file_type(&dir, is_dir).expect("dir"));
    }

    #[rstest]
    fn detects_file(
        file_paths: (
            tempfile::TempDir,
            Utf8PathBuf,
            Utf8PathBuf,
            Utf8PathBuf,
            Utf8PathBuf,
            Utf8PathBuf,
            Utf8PathBuf,
        ),
    ) {
        let (_, _, file, _, _, _, _) = file_paths;
        assert!(is_file_type(&file, is_file).expect("file"));
    }

    #[rstest]
    fn detects_symlink(
        file_paths: (
            tempfile::TempDir,
            Utf8PathBuf,
            Utf8PathBuf,
            Utf8PathBuf,
            Utf8PathBuf,
            Utf8PathBuf,
            Utf8PathBuf,
        ),
    ) {
        let (_, _, file, link, _, _, _) = file_paths;
        assert!(is_file_type(&link, is_symlink).expect("link"));
        assert!(!is_file_type(&file, is_symlink).expect("file"));
    }

    #[cfg(unix)]
    #[rstest]
    fn detects_pipe(
        file_paths: (
            tempfile::TempDir,
            Utf8PathBuf,
            Utf8PathBuf,
            Utf8PathBuf,
            Utf8PathBuf,
            Utf8PathBuf,
            Utf8PathBuf,
        ),
    ) {
        let (_, _, _, _, fifo, _, _) = file_paths;
        assert!(is_file_type(&fifo, is_fifo).expect("fifo"));
    }

    #[cfg(unix)]
    #[rstest]
    fn detects_block_device(
        file_paths: (
            tempfile::TempDir,
            Utf8PathBuf,
            Utf8PathBuf,
            Utf8PathBuf,
            Utf8PathBuf,
            Utf8PathBuf,
            Utf8PathBuf,
        ),
    ) {
        let (_, _, _, _, _, bdev, _) = file_paths;
        if !bdev.as_std_path().exists() {
            eprintln!("block device fixture not found; skipping");
            return;
        }
        assert!(is_file_type(&bdev, is_block_device).expect("block"));
    }

    #[cfg(unix)]
    #[rstest]
    fn detects_char_device(
        file_paths: (
            tempfile::TempDir,
            Utf8PathBuf,
            Utf8PathBuf,
            Utf8PathBuf,
            Utf8PathBuf,
            Utf8PathBuf,
            Utf8PathBuf,
        ),
    ) {
        let (_, _, _, _, _, _, cdev) = file_paths;
        assert!(is_file_type(&cdev, is_char_device).expect("char"));
    }

    #[cfg(unix)]
    #[rstest]
    fn detects_device(
        file_paths: (
            tempfile::TempDir,
            Utf8PathBuf,
            Utf8PathBuf,
            Utf8PathBuf,
            Utf8PathBuf,
            Utf8PathBuf,
            Utf8PathBuf,
        ),
    ) {
        let (_, _, _, _, _, _, cdev) = file_paths;
        assert!(is_file_type(&cdev, is_device).expect("device"));
    }

    #[rstest]
    fn nonexistent_path_is_false() {
        let temp = tempdir().expect("tempdir");
        let missing =
            Utf8PathBuf::from_path_buf(temp.path().join("missing")).expect("utf8 missing path");
        assert!(!is_file_type(&missing, is_file).expect("missing"));
    }
}
