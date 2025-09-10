use camino::Utf8Path;
use cap_std::{
    ambient_authority,
    fs::{self, FileTypeExt},
    fs_utf8::Dir,
};
use minijinja::{Environment, Error, ErrorKind};
use std::io;

/// Register standard library helpers with the Jinja environment.
pub fn register(env: &mut Environment<'_>) {
    env.add_test("dir", |path: String| {
        is_file_type(Utf8Path::new(&path), |ft| ft.is_dir())
    });
    env.add_test("file", |path: String| {
        is_file_type(Utf8Path::new(&path), |ft| ft.is_file())
    });
    env.add_test("symlink", |path: String| {
        is_file_type(Utf8Path::new(&path), |ft| ft.is_symlink())
    });
    env.add_test("pipe", |path: String| {
        is_file_type(Utf8Path::new(&path), |ft| ft.is_fifo())
    });
    env.add_test("device", |path: String| {
        is_file_type(Utf8Path::new(&path), |ft| {
            ft.is_block_device() || ft.is_char_device()
        })
    });
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
        (temp, dir, file, link, fifo)
    }

    #[rstest]
    fn detects_dir(
        file_paths: (
            tempfile::TempDir,
            Utf8PathBuf,
            Utf8PathBuf,
            Utf8PathBuf,
            Utf8PathBuf,
        ),
    ) {
        let (_, dir, _, _, _) = file_paths;
        assert!(is_file_type(&dir, |ft| ft.is_dir()).expect("dir"));
    }

    #[rstest]
    fn detects_file(
        file_paths: (
            tempfile::TempDir,
            Utf8PathBuf,
            Utf8PathBuf,
            Utf8PathBuf,
            Utf8PathBuf,
        ),
    ) {
        let (_, _, file, _, _) = file_paths;
        assert!(is_file_type(&file, |ft| ft.is_file()).expect("file"));
    }

    #[rstest]
    fn detects_symlink(
        file_paths: (
            tempfile::TempDir,
            Utf8PathBuf,
            Utf8PathBuf,
            Utf8PathBuf,
            Utf8PathBuf,
        ),
    ) {
        let (_, _, file, link, _) = file_paths;
        assert!(is_file_type(&link, |ft| ft.is_symlink()).expect("link"));
        assert!(!is_file_type(&file, |ft| ft.is_symlink()).expect("file"));
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
        ),
    ) {
        let (_, _, _, _, fifo) = file_paths;
        assert!(is_file_type(&fifo, |ft| ft.is_fifo()).expect("fifo"));
    }

    #[cfg(unix)]
    #[rstest]
    fn detects_device() {
        assert!(
            is_file_type(Utf8Path::new("/dev/null"), |ft| ft.is_block_device()
                || ft.is_char_device(),)
            .expect("device"),
        );
    }

    #[rstest]
    fn nonexistent_path_is_false() {
        assert!(!is_file_type(Utf8Path::new("/no/such/path"), |ft| ft.is_file()).expect("missing"),);
    }
}
