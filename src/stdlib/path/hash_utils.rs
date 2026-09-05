//! Hash utilities for stdlib path filters.
//!
//! Streams SHA-256 and SHA-512 digests via cap-std handles,
//! enables SHA-1 and MD5 behind the `legacy-digests` feature,
//! and always returns lowercase hexadecimal output.
use camino::Utf8Path;
use digest::Digest;
#[cfg(feature = "legacy-digests")]
use md5::Md5;
use minijinja::{Error, ErrorKind};
#[cfg(feature = "legacy-digests")]
use sha1::Sha1;
use sha2::{Sha256, Sha512};

use super::fs_utils::{self, FileReadLimits};
use crate::hex::to_lower_hex;
use crate::localization::{self, keys};

/// Hash the file at `path` with the named algorithm, returning lowercase hex.
///
/// # Errors
///
/// Returns an error when the algorithm is unsupported, is gated behind the
/// `legacy-digests` feature when unavailable, or the file cannot be read.
pub(super) fn compute_hash(
    path: &Utf8Path,
    alg: &str,
    limits: &FileReadLimits,
) -> Result<String, Error> {
    if alg.eq_ignore_ascii_case("sha256") {
        hash_stream::<Sha256>(path, limits)
    } else if alg.eq_ignore_ascii_case("sha512") {
        hash_stream::<Sha512>(path, limits)
    } else if alg.eq_ignore_ascii_case("sha1") {
        #[cfg(feature = "legacy-digests")]
        {
            hash_stream::<Sha1>(path, limits)
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
            hash_stream::<Md5>(path, limits)
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
/// Hash the file and truncate the hex digest to `len` characters.
///
/// # Errors
///
/// Returns an error when `alg` is unsupported, when a legacy algorithm is
/// unavailable without the `legacy-digests` feature, or when the file cannot
/// be opened or read.
pub(super) fn compute_digest(
    path: &Utf8Path,
    len: usize,
    alg: &str,
    limits: &FileReadLimits,
) -> Result<String, Error> {
    let mut hash = compute_hash(path, alg, limits)?;
    if len < hash.len() {
        hash.truncate(len);
    }
    Ok(hash)
}

/// Stream the file through a hasher in fixed-size chunks, returning lowercase hex.
///
/// # Errors
///
/// Returns a template error when the file cannot be opened or a chunk cannot
/// be read, or when the file exceeds the configured byte budget.
fn hash_stream<H>(path: &Utf8Path, limits: &FileReadLimits) -> Result<String, Error>
where
    H: Digest,
{
    let mut file = fs_utils::open_file_checked(path, limits)?;
    let mut hasher = H::new();
    let mut buffer = [0_u8; 8192];
    let mut state = fs_utils::BoundedRead::new(limits.max_bytes);
    loop {
        let Some(chunk) = fs_utils::read_bounded_chunk(&mut state, &mut file, &mut buffer, path)?
        else {
            break;
        };
        hasher.update(chunk);
    }
    Ok(to_lower_hex(&hasher.finalize()))
}

#[cfg(test)]
mod tests {
    //! Tests for the chunked digest-streaming loop.
    //!
    //! `hash_stream` feeds the hasher from a fixed 8192-byte buffer because the
    //! `RustCrypto` 0.11 hashers no longer implement `io::Write`, so `io::copy`
    //! is unavailable. The loop is the part that could silently drop, reorder,
    //! or double-count a chunk, and only inputs larger than the buffer exercise
    //! more than one iteration of it.

    use anyhow::{Result, anyhow, ensure};
    use camino::Utf8PathBuf;
    use cap_std::{ambient_authority, fs_utf8::Dir};
    use rstest::rstest;
    use sha2::{Digest, Sha256};
    use tempfile::TempDir;

    use super::super::fs_utils::FileReadLimits;
    use super::{compute_hash, to_lower_hex};

    /// Name of the fixture file staged inside the temporary directory.
    const FIXTURE_NAME: &str = "payload";

    /// Write `payload` to a temporary file and return the directory guard
    /// alongside the file's path.
    ///
    /// The write goes through a `cap_std` directory capability rather than
    /// ambient `std::fs`, matching the convention the sibling test modules
    /// follow. The guard must outlive the returned path: dropping it removes
    /// the file.
    fn fixture(payload: &[u8]) -> Result<(TempDir, Utf8PathBuf)> {
        let dir = tempfile::tempdir()?;
        let root = Utf8PathBuf::from_path_buf(dir.path().to_path_buf())
            .map_err(|path| anyhow!("temporary path is not valid UTF-8: {path:?}"))?;
        let handle = Dir::open_ambient_dir(&root, ambient_authority())?;
        handle.write(FIXTURE_NAME, payload)?;
        Ok((dir, root.join(FIXTURE_NAME)))
    }

    /// A repeating, non-constant byte pattern of `size` bytes.
    ///
    /// Cycling `0..=u8::MAX` means a chunk dropped, reordered, or counted twice
    /// changes the digest. A constant fill would not.
    fn patterned(size: usize) -> Vec<u8> {
        (0..=u8::MAX).cycle().take(size).collect()
    }

    #[rstest]
    #[case::empty(0)]
    #[case::single_read(4)]
    #[case::exactly_one_buffer(8192)]
    #[case::spans_two_reads(8193)]
    #[case::spans_several_reads(70_000)]
    fn streamed_digest_matches_a_one_shot_digest(#[case] size: usize) -> Result<()> {
        let payload = patterned(size);
        let (_dir, file) = fixture(&payload)?;

        let streamed = compute_hash(
            &file,
            "sha256",
            &FileReadLimits {
                max_bytes: u64::MAX,
                follow_symlinks: false,
            },
        )?;
        let one_shot = to_lower_hex(&Sha256::digest(&payload));

        ensure!(
            streamed == one_shot,
            "streamed digest of {size} bytes was {streamed} but a one-shot digest is {one_shot}"
        );
        Ok(())
    }

    /// Anchor the streaming path to a published vector, so the cross-check
    /// above cannot pass by agreeing on a wrong digest.
    #[rstest]
    fn streamed_digest_matches_the_known_vector_for_abc() -> Result<()> {
        let (_dir, file) = fixture(b"abc")?;
        let expected = concat!(
            "ba7816bf8f01cfea414140de5dae2223",
            "b00361a396177a9cb410ff61f20015ad",
        );
        let digest = compute_hash(
            &file,
            "sha256",
            &FileReadLimits {
                max_bytes: u64::MAX,
                follow_symlinks: false,
            },
        )?;
        ensure!(
            digest == expected,
            "expected the published digest {expected} but streamed {digest}"
        );
        Ok(())
    }

    mod properties {
        //! Property tests for the chunked streaming loop.
        //!
        //! The cases above pin sizes chosen to straddle the 8192-byte read
        //! buffer. They cannot cover how the loop behaves at an arbitrary
        //! offset within a chunk, which is where a partial final read or a
        //! mishandled boundary would hide. Generating the length instead lets
        //! the partition vary freely, including the awkward remainders either
        //! side of a buffer boundary.

        use proptest::prelude::*;
        use sha2::{Digest, Sha256};

        use super::super::fs_utils::FileReadLimits;
        use super::{compute_hash, fixture, to_lower_hex};

        proptest! {
            /// Streaming a payload of any length agrees with a one-shot digest.
            ///
            /// The range spans several buffer fills so the generated lengths
            /// exercise both exact multiples and partial trailing reads.
            #[test]
            fn streamed_digest_matches_a_one_shot_digest_for_any_length(
                payload in prop::collection::vec(any::<u8>(), 0..20_000),
            ) {
                let (_dir, file) = fixture(&payload).expect("stage the payload");

                let streamed = compute_hash(
                    &file,
                    "sha256",
                    &FileReadLimits { max_bytes: u64::MAX, follow_symlinks: false },
                ).expect("hash the payload");
                let one_shot = to_lower_hex(&Sha256::digest(&payload));

                prop_assert_eq!(streamed, one_shot);
            }
        }
    }
}
