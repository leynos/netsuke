use std::{
    env,
    fmt::Write as FmtWrite,
    io::{self, Read},
};

use camino::{Utf8Path, Utf8PathBuf};
use cap_std::{ambient_authority, fs, fs_utf8::Dir};
use digest::Digest;
#[cfg(feature = "legacy-digests")]
use md5::Md5;
use minijinja::{Environment, Error, ErrorKind};

mod io_helpers;

use io_helpers::io_to_error;
#[cfg(feature = "legacy-digests")]
use sha1::Sha1;
use sha2::{Sha256, Sha512};

pub(super) fn register_filters(env: &mut Environment<'_>) {
    env.add_filter("basename", |raw: String| -> Result<String, Error> {
        Ok(basename(Utf8Path::new(&raw)))
    });
    env.add_filter("dirname", |raw: String| -> Result<String, Error> {
        Ok(dirname(Utf8Path::new(&raw)))
    });
    env.add_filter(
        "with_suffix",
        |raw: String,
         suffix: String,
         count: Option<usize>,
         sep: Option<String>|
         -> Result<String, Error> {
            let count = count.unwrap_or(1);
            let sep = sep.unwrap_or_else(|| ".".to_string());
            with_suffix(Utf8Path::new(&raw), &suffix, count, &sep).map(Utf8PathBuf::into_string)
        },
    );
    env.add_filter(
        "relative_to",
        |raw: String, root: String| -> Result<String, Error> {
            relative_to(Utf8Path::new(&raw), Utf8Path::new(&root))
        },
    );
    env.add_filter("realpath", |raw: String| -> Result<String, Error> {
        canonicalize_any(Utf8Path::new(&raw)).map(Utf8PathBuf::into_string)
    });
    env.add_filter("expanduser", |raw: String| -> Result<String, Error> {
        expanduser(&raw)
    });
    env.add_filter("size", |raw: String| -> Result<u64, Error> {
        file_size(Utf8Path::new(&raw))
    });
    env.add_filter(
        "contents",
        |raw: String, encoding: Option<String>| -> Result<String, Error> {
            let encoding = encoding.unwrap_or_else(|| "utf-8".to_string());
            read_text(Utf8Path::new(&raw), &encoding)
        },
    );
    env.add_filter("linecount", |raw: String| -> Result<usize, Error> {
        linecount(Utf8Path::new(&raw))
    });
    env.add_filter(
        "hash",
        |raw: String, alg: Option<String>| -> Result<String, Error> {
            let alg = alg.unwrap_or_else(|| "sha256".to_string());
            compute_hash(Utf8Path::new(&raw), &alg)
        },
    );
    env.add_filter(
        "digest",
        |raw: String, len: Option<usize>, alg: Option<String>| -> Result<String, Error> {
            let len = len.unwrap_or(8);
            let alg = alg.unwrap_or_else(|| "sha256".to_string());
            compute_digest(Utf8Path::new(&raw), len, &alg)
        },
    );
}

pub(super) fn file_type_matches<F>(path: &Utf8Path, predicate: F) -> Result<bool, Error>
where
    F: Fn(fs::FileType) -> bool,
{
    let (dir, name, _) = match parent_dir(path) {
        Ok(parts) => parts,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(io_to_error(path, "open directory", err)),
    };
    match dir.symlink_metadata(Utf8Path::new(&name)) {
        Ok(md) => Ok(predicate(md.file_type())),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(io_to_error(path, "stat", err)),
    }
}

fn normalise_parent(parent: Option<&Utf8Path>) -> Utf8PathBuf {
    parent
        .filter(|p| !p.as_str().is_empty())
        .map_or_else(|| Utf8PathBuf::from("."), Utf8Path::to_path_buf)
}

fn dir_and_basename(path: &Utf8Path) -> (Utf8PathBuf, String) {
    let dir = normalise_parent(path.parent());
    let name = path.file_name().map_or_else(|| ".".into(), str::to_string);
    (dir, name)
}

fn basename(path: &Utf8Path) -> String {
    path.file_name().unwrap_or(path.as_str()).to_string()
}

fn parent_dir(path: &Utf8Path) -> Result<(Dir, String, Utf8PathBuf), io::Error> {
    let (dir_path, name) = dir_and_basename(path);
    let dir = Dir::open_ambient_dir(&dir_path, ambient_authority())?;
    Ok((dir, name, dir_path))
}

fn open_parent_dir(path: &Utf8Path) -> Result<(Dir, String, Utf8PathBuf), Error> {
    parent_dir(path).map_err(|err| io_to_error(path, "open directory", err))
}

fn is_root(path: &Utf8Path) -> bool {
    path.parent().is_none() && path.file_name().is_none() && !path.as_str().is_empty()
}

fn current_dir_utf8() -> Result<Utf8PathBuf, io::Error> {
    let cwd = env::current_dir()?;
    Utf8PathBuf::from_path_buf(cwd)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "current dir is not valid UTF-8"))
}

fn canonicalize_any(path: &Utf8Path) -> Result<Utf8PathBuf, Error> {
    if path.as_str().is_empty() || path == Utf8Path::new(".") {
        return current_dir_utf8()
            .map_err(|err| io_to_error(Utf8Path::new("."), "canonicalise", err));
    }
    if is_root(path) {
        return Ok(path.to_path_buf());
    }
    let (dir, name, dir_path) = open_parent_dir(path)?;
    let canonical_child = dir
        .canonicalize(Utf8Path::new(&name))
        .map_err(|err| io_to_error(path, "canonicalise", err))?;
    if name == "." {
        return canonicalize_any(&dir_path);
    }
    let mut parent = if dir_path.as_str() == "." {
        current_dir_utf8().map_err(|err| io_to_error(Utf8Path::new("."), "canonicalise", err))?
    } else {
        canonicalize_any(&dir_path)?
    };
    parent.push(&canonical_child);
    Ok(parent)
}

fn dirname(path: &Utf8Path) -> String {
    normalise_parent(path.parent()).into_string()
}

fn with_suffix(
    path: &Utf8Path,
    suffix: &str,
    count: usize,
    sep: &str,
) -> Result<Utf8PathBuf, Error> {
    if sep.is_empty() {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            "with_suffix requires a non-empty separator",
        ));
    }
    let mut base = path.to_path_buf();
    let name = base.file_name().map(str::to_owned).unwrap_or_default();
    if !name.is_empty() {
        base.pop();
    }
    let mut stem = name;
    let mut removed = 0;
    while removed < count {
        if let Some(idx) = stem.rfind(sep) {
            stem.truncate(idx);
            removed += 1;
        } else {
            break;
        }
    }
    stem.push_str(suffix);
    let replacement = Utf8PathBuf::from(stem);
    base.push(&replacement);
    Ok(base)
}

fn relative_to(path: &Utf8Path, root: &Utf8Path) -> Result<String, Error> {
    path.strip_prefix(root)
        .map(|p| p.as_str().to_string())
        .map_err(|_| {
            Error::new(
                ErrorKind::InvalidOperation,
                format!("{path} is not relative to {root}"),
            )
        })
}

fn is_user_specific_expansion(stripped: &str) -> bool {
    matches!(
        stripped.chars().next(),
        Some(first) if first != '/' && first != std::path::MAIN_SEPARATOR
    )
}

fn resolve_home() -> Result<String, Error> {
    home_from_env().ok_or_else(|| {
        Error::new(
            ErrorKind::InvalidOperation,
            "cannot expand ~: no home directory environment variables are set",
        )
    })
}

fn expanduser(raw: &str) -> Result<String, Error> {
    if let Some(stripped) = raw.strip_prefix('~') {
        if is_user_specific_expansion(stripped) {
            return Err(Error::new(
                ErrorKind::InvalidOperation,
                "user-specific ~ expansion is unsupported",
            ));
        }
        let home = resolve_home()?;
        Ok(format!("{home}{stripped}"))
    } else {
        Ok(raw.to_string())
    }
}

#[cfg(windows)]
fn home_from_env() -> Option<String> {
    env::var("HOME")
        .or_else(|_| env::var("USERPROFILE"))
        .ok()
        .or_else(
            || match (env::var("HOMEDRIVE").ok(), env::var("HOMEPATH").ok()) {
                (Some(drive), Some(path)) if !path.is_empty() => Some(format!("{drive}{path}")),
                _ => env::var("HOMESHARE").ok(),
            },
        )
}

#[cfg(not(windows))]
fn home_from_env() -> Option<String> {
    env::var("HOME").or_else(|_| env::var("USERPROFILE")).ok()
}

fn file_size(path: &Utf8Path) -> Result<u64, Error> {
    let (dir, name, _) = open_parent_dir(path)?;
    dir.metadata(Utf8Path::new(&name))
        .map(|meta| meta.len())
        .map_err(|err| io_to_error(path, "stat", err))
}

fn read_text(path: &Utf8Path, encoding: &str) -> Result<String, Error> {
    let encoding = encoding.to_ascii_lowercase();
    if encoding != "utf-8" && encoding != "utf8" {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            format!("unsupported encoding '{encoding}'"),
        ));
    }
    let (dir, name, _) = open_parent_dir(path)?;
    dir.read_to_string(Utf8Path::new(&name))
        .map_err(|err| io_to_error(path, "read", err))
}

fn linecount(path: &Utf8Path) -> Result<usize, Error> {
    let text = read_text(path, "utf-8")?;
    Ok(text.lines().count())
}

fn compute_hash(path: &Utf8Path, alg: &str) -> Result<String, Error> {
    match alg.to_ascii_lowercase().as_str() {
        "sha256" => hash_stream::<Sha256>(path),
        "sha512" => hash_stream::<Sha512>(path),
        "sha1" => {
            #[cfg(feature = "legacy-digests")]
            {
                hash_stream::<Sha1>(path)
            }
            #[cfg(not(feature = "legacy-digests"))]
            {
                Err(Error::new(
                    ErrorKind::InvalidOperation,
                    "unsupported hash algorithm 'sha1' (enable feature 'legacy-digests')"
                        .to_string(),
                ))
            }
        }
        "md5" => {
            #[cfg(feature = "legacy-digests")]
            {
                hash_stream::<Md5>(path)
            }
            #[cfg(not(feature = "legacy-digests"))]
            {
                Err(Error::new(
                    ErrorKind::InvalidOperation,
                    "unsupported hash algorithm 'md5' (enable feature 'legacy-digests')"
                        .to_string(),
                ))
            }
        }
        other => Err(Error::new(
            ErrorKind::InvalidOperation,
            format!("unsupported hash algorithm '{other}'"),
        )),
    }
}

fn hash_stream<H>(path: &Utf8Path) -> Result<String, Error>
where
    H: Digest,
{
    let (dir, name, _) = open_parent_dir(path)?;
    let mut opts = fs::OpenOptions::new();
    opts.read(true);
    let mut file = dir
        .open_with(Utf8Path::new(&name), &opts)
        .map_err(|err| io_to_error(path, "open file", err))?;
    let mut hasher = H::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|err| io_to_error(path, "read", err))?;
        if read == 0 {
            break;
        }
        let chunk = buffer.get(..read).unwrap_or(&[]);
        hasher.update(chunk);
    }
    let digest = hasher.finalize();
    Ok(encode_hex(digest.as_slice()))
}

fn compute_digest(path: &Utf8Path, len: usize, alg: &str) -> Result<String, Error> {
    let mut hash = compute_hash(path, alg)?;
    if len < hash.len() {
        hash.truncate(len);
    }
    Ok(hash)
}

fn encode_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(&mut out, "{b:02x}");
    }
    out
}
