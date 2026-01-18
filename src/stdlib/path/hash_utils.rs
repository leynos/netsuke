//! Hash utilities for stdlib path filters.
//!
//! Streams SHA-256 and SHA-512 digests via cap-std handles,
//! enables SHA-1 and MD5 behind the `legacy-digests` feature,
//! and always returns lowercase hexadecimal output.
use std::io::Read;

use camino::Utf8Path;
use digest::Digest;
#[cfg(feature = "legacy-digests")]
use md5::Md5;
use minijinja::{Error, ErrorKind};
#[cfg(feature = "legacy-digests")]
use sha1::Sha1;
use sha2::{Sha256, Sha512};

use super::{fs_utils, io_helpers::io_to_error};
use crate::localization::{self, keys};

pub(super) fn compute_hash(path: &Utf8Path, alg: &str) -> Result<String, Error> {
    if alg.eq_ignore_ascii_case("sha256") {
        hash_stream::<Sha256>(path)
    } else if alg.eq_ignore_ascii_case("sha512") {
        hash_stream::<Sha512>(path)
    } else if alg.eq_ignore_ascii_case("sha1") {
        #[cfg(feature = "legacy-digests")]
        {
            hash_stream::<Sha1>(path)
        }
        #[cfg(not(feature = "legacy-digests"))]
        {
            Err(Error::new(
                ErrorKind::InvalidOperation,
                localization::message(keys::STDLIB_PATH_HASH_UNSUPPORTED_ALGORITHM_LEGACY)
                    .with_arg("algorithm", "sha1")
                    .with_arg("feature", "legacy-digests")
                    .to_string(),
            ))
        }
    } else if alg.eq_ignore_ascii_case("md5") {
        #[cfg(feature = "legacy-digests")]
        {
            hash_stream::<Md5>(path)
        }
        #[cfg(not(feature = "legacy-digests"))]
        {
            Err(Error::new(
                ErrorKind::InvalidOperation,
                localization::message(keys::STDLIB_PATH_HASH_UNSUPPORTED_ALGORITHM_LEGACY)
                    .with_arg("algorithm", "md5")
                    .with_arg("feature", "legacy-digests")
                    .to_string(),
            ))
        }
    } else {
        Err(Error::new(
            ErrorKind::InvalidOperation,
            localization::message(keys::STDLIB_PATH_HASH_UNSUPPORTED_ALGORITHM)
                .with_arg("algorithm", alg)
                .to_string(),
        ))
    }
}
pub(super) fn compute_digest(path: &Utf8Path, len: usize, alg: &str) -> Result<String, Error> {
    let mut hash = compute_hash(path, alg)?;
    if len < hash.len() {
        hash.truncate(len);
    }
    Ok(hash)
}

fn hash_stream<H>(path: &Utf8Path) -> Result<String, Error>
where
    H: Digest,
{
    let mut file = fs_utils::open_file(path)?;
    let mut hasher = H::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = file.read(&mut buffer).map_err(|err| {
            io_to_error(
                path,
                &localization::message(keys::STDLIB_PATH_ACTION_READ),
                err,
            )
        })?;
        if read == 0 {
            break;
        }
        let chunk = buffer.get(..read).unwrap_or_else(|| {
            debug_assert!(false, "read beyond buffer capacity: {read} bytes");
            buffer.as_slice()
        });
        hasher.update(chunk);
    }
    Ok(encode_hex(&hasher.finalize()))
}

fn encode_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        if let Err(err) = write!(&mut out, "{byte:02x}") {
            debug_assert!(false, "format hex byte failed: {err}");
        }
    }
    out
}
