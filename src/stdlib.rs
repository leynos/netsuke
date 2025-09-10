use camino::Utf8Path;
#[cfg(unix)]
use cap_std::fs::FileTypeExt;
use cap_std::{ambient_authority, fs, fs_utf8::Dir};
use minijinja::{Environment, Error, ErrorKind};
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

    for (name, pred) in TESTS {
        env.add_test(*name, move |path: String| {
            is_file_type(Utf8Path::new(&path), *pred)
        });
    }
}

/// Determine whether `path` matches the given file type predicate.
///
/// Returns `Ok(false)` if the path does not exist.
fn is_file_type(path: &Utf8Path, predicate: fn(fs::FileType) -> bool) -> Result<bool, Error> {
    let (dir_path, file_name) = path.parent().map_or_else(
        || (Utf8Path::new("."), path.as_str()),
        |parent| (parent, path.file_name().unwrap_or("")),
    );
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
    use camino::{Utf8Path, Utf8PathBuf};
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
        let bdev = root.join("b");
        let cdev = root.join("c");
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
        #[cfg(unix)]
        mknodat(
            &handle,
            "b",
            FileType::BlockDevice,
            Mode::RUSR | Mode::WUSR,
            Dev::default(),
        )
        .expect("block");
        #[cfg(unix)]
        mknodat(
            &handle,
            "c",
            FileType::CharacterDevice,
            Mode::RUSR | Mode::WUSR,
            Dev::default(),
        )
        .expect("char");
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
        assert!(!is_file_type(Utf8Path::new("/no/such/path"), is_file).expect("missing"));
    }
}
