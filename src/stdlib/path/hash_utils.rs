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

pub(super) fn compute_hash(path: &Utf8Path, alg: &str) -> Result<String, Error> {
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
                    "unsupported hash algorithm 'sha1' (enable feature 'legacy-digests')",
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
                    "unsupported hash algorithm 'md5' (enable feature 'legacy-digests')",
                ))
            }
        }
        other => Err(Error::new(
            ErrorKind::InvalidOperation,
            format!("unsupported hash algorithm '{other}'"),
        )),
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
        let read = file
            .read(&mut buffer)
            .map_err(|err| io_to_error(path, "read", err))?;
        if read == 0 {
            break;
        }
        let chunk = buffer.get(..read).expect("read beyond buffer capacity");
        hasher.update(chunk);
    }
    Ok(encode_hex(&hasher.finalize()))
}

fn encode_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}
