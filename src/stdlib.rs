use minijinja::{Environment, Error, ErrorKind};
use std::os::unix::fs::FileTypeExt;
use std::{fs, io};

/// Register standard library helpers with the Jinja environment.
pub fn register(env: &mut Environment<'_>) {
    env.add_test("dir", |path: String| is_file_type(&path, |ft| ft.is_dir()));
    env.add_test("file", |path: String| {
        is_file_type(&path, |ft| ft.is_file())
    });
    env.add_test("symlink", |path: String| {
        is_file_type(&path, |ft| ft.is_symlink())
    });
    env.add_test("pipe", |path: String| {
        is_file_type(&path, |ft| ft.is_fifo())
    });
    env.add_test("device", |path: String| {
        is_file_type(&path, |ft| ft.is_block_device() || ft.is_char_device())
    });
}

/// Determine whether `path` matches the given file type predicate.
///
/// Returns `Ok(false)` if the path does not exist.
fn is_file_type(path: &str, predicate: fn(fs::FileType) -> bool) -> Result<bool, Error> {
    match fs::symlink_metadata(path) {
        Ok(md) => Ok(predicate(md.file_type())),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(Error::new(ErrorKind::InvalidOperation, err.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nix::{sys::stat::Mode, unistd::mkfifo};
    use rstest::{fixture, rstest};
    use std::os::unix::fs::FileTypeExt;
    use std::{os::unix::fs as unix_fs, path::PathBuf};
    use tempfile::tempdir;

    #[fixture]
    fn file_paths() -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf, PathBuf) {
        let temp = tempdir().expect("tempdir");
        let dir = temp.path().join("d");
        fs::create_dir(&dir).expect("dir");
        let file = temp.path().join("f");
        fs::write(&file, b"x").expect("file");
        let link = temp.path().join("s");
        unix_fs::symlink(&file, &link).expect("symlink");
        let fifo = temp.path().join("p");
        mkfifo(&fifo, Mode::S_IRUSR | Mode::S_IWUSR).expect("fifo");
        (temp, dir, file, link, fifo)
    }

    #[rstest]
    fn detects_dir(file_paths: (tempfile::TempDir, PathBuf, PathBuf, PathBuf, PathBuf)) {
        let (_, dir, _, _, _) = file_paths;
        assert!(is_file_type(dir.to_str().expect("utf8"), |ft| ft.is_dir()).expect("dir"));
    }

    #[rstest]
    fn detects_file(file_paths: (tempfile::TempDir, PathBuf, PathBuf, PathBuf, PathBuf)) {
        let (_, _, file, _, _) = file_paths;
        assert!(is_file_type(file.to_str().expect("utf8"), |ft| ft.is_file()).expect("file"));
    }

    #[rstest]
    fn detects_symlink(file_paths: (tempfile::TempDir, PathBuf, PathBuf, PathBuf, PathBuf)) {
        let (_, _, file, link, _) = file_paths;
        assert!(is_file_type(link.to_str().expect("utf8"), |ft| ft.is_symlink()).expect("link"));
        assert!(!is_file_type(file.to_str().expect("utf8"), |ft| ft.is_symlink()).expect("file"));
    }

    #[rstest]
    fn detects_pipe(file_paths: (tempfile::TempDir, PathBuf, PathBuf, PathBuf, PathBuf)) {
        let (_, _, _, _, fifo) = file_paths;
        assert!(is_file_type(fifo.to_str().expect("utf8"), |ft| ft.is_fifo()).expect("fifo"));
    }

    #[rstest]
    fn detects_device() {
        assert!(
            is_file_type("/dev/null", |ft| ft.is_block_device()
                || ft.is_char_device())
            .expect("device")
        );
    }

    #[rstest]
    fn nonexistent_path_is_false() {
        assert!(!is_file_type("/no/such/path", |ft| ft.is_file()).expect("missing"));
    }
}
