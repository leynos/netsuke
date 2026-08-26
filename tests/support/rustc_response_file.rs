//! Builds `rustc` response files for the direct-compile UI harnesses.
//!
//! Why this exists: Cargo 1.99 gives every crate its own artefact directory
//! rather than one shared `deps/`, so a harness that must reach every
//! dependency passes one `-L dependency=<directory>` pair per crate. The
//! split-build regression test compounds that with long, unique temporary
//! roots. On Windows the resulting `CreateProcessW` command line exceeds the
//! 32,767-character limit and the spawn fails before `rustc` runs at all, with
//! `Os { code: 206, kind: InvalidFilename }`. Every one of those directories is
//! required — dropping any of them reintroduces `E0463` — so the fix is to stop
//! putting them on the command line, not to shorten the list.
//!
//! `rustc` reads arguments from a file named `@<path>`, one argument per line,
//! UTF-8 encoded. That moves the whole argument vector off the command line
//! and leaves the spawn well under any platform limit.
//!
//! Scope, deliberately narrow: rendering an argument vector into response-file
//! text and writing it. Nothing here spawns a process, chooses arguments, or
//! knows what a compilation needs.
//!
//! Reuse policy: include this module from a `tests/*.rs` binary that invokes
//! `rustc` directly with an argument list whose length is not bounded by the
//! source. A harness passing a fixed handful of arguments does not need it.
//!
//! Integration tests under `tests/` compile as independent crates, so there is
//! no library to share through. The module lives in a subdirectory, which Cargo
//! does not auto-discover as a test target, and each consumer includes it with
//! `#[path = "support/rustc_response_file.rs"] mod rustc_response_file;`.
//!
//! Every helper is exercised by this module's own unit tests, which run once
//! per including crate. That is what keeps a consumer using only part of the
//! surface from tripping `dead_code`.

use std::io;
use std::path::{Path, PathBuf};

/// Renders `args` as response-file text: one argument per line.
///
/// `rustc` splits a response file on line boundaries and performs no quoting or
/// escaping, so an argument containing a newline would silently become two
/// arguments. That cannot arise from the paths these harnesses pass, but it
/// would corrupt the compilation invisibly if it ever did, so it is rejected
/// here rather than diagnosed later as a baffling `rustc` error.
///
/// # Errors
///
/// Returns an error when any argument contains a newline.
///
/// # Examples
///
/// ```
/// let text = render(&["--edition=2024".to_owned(), "-L".to_owned()])
///     .expect("newline-free arguments");
/// assert_eq!(text, "--edition=2024\n-L\n");
/// ```
pub fn render(args: &[String]) -> io::Result<String> {
    if let Some(bad) = args.iter().find(|arg| arg.contains('\n')) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("a response-file argument cannot contain a newline: {bad:?}"),
        ));
    }
    let mut text = String::new();
    for arg in args {
        text.push_str(arg);
        text.push('\n');
    }
    Ok(text)
}

/// Writes `args` to `<dir>/<file_name>` and returns the `@path` argument.
///
/// The returned string is the single argument to hand `rustc`. `dir` is
/// normally the harness's existing output `TempDir`, which must outlive the
/// `rustc` invocation: the response file is read at spawn time, so dropping the
/// directory first would delete the file before `rustc` opens it.
///
/// # Errors
///
/// Returns an error when an argument contains a newline, when the path is not
/// valid UTF-8 (`rustc` requires a UTF-8 response file path), or when the write
/// fails.
///
/// # Examples
///
/// ```no_run
/// let dir = tempfile::tempdir().expect("temp dir");
/// let arg = write(dir.path(), "ui.args", &["--edition=2024".to_owned()])
///     .expect("write the response file");
/// assert!(arg.starts_with('@'));
/// ```
pub fn write(dir: &Path, file_name: &str, args: &[String]) -> io::Result<String> {
    let path: PathBuf = dir.join(file_name);
    let text = render(args)?;
    test_support::fs::write(&path, text.as_bytes()).map_err(|error| {
        io::Error::other(format!("write response file {}: {error}", path.display()))
    })?;
    let path_str = path.to_str().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("response file path is not UTF-8: {}", path.display()),
        )
    })?;
    Ok(format!("@{path_str}"))
}

#[cfg(test)]
mod tests {
    //! Unit tests for the response-file builder.
    //!
    //! These also keep every helper used from each including crate, so a
    //! consumer needing only part of the surface does not trip `dead_code`.
    //!
    //! The command-line-length failure these helpers exist to prevent is
    //! Windows-specific and cannot be reproduced on the machines that run most
    //! of this suite, so the contract asserted here is the file's *shape* —
    //! which holds everywhere — rather than a host-specific spawn.

    use super::{render, write};
    use proptest::prelude::*;

    fn owned(args: &[&str]) -> Vec<String> {
        args.iter().map(|arg| (*arg).to_owned()).collect()
    }

    #[test]
    fn each_argument_occupies_its_own_line() {
        let args = owned(&["--edition=2024", "--crate-type=bin", "--emit=metadata"]);
        let text = render(&args).expect("newline-free arguments render");
        assert_eq!(
            text.lines().collect::<Vec<_>>(),
            ["--edition=2024", "--crate-type=bin", "--emit=metadata"],
            "rustc splits a response file on line boundaries, one argument per line"
        );
        assert!(
            text.ends_with('\n'),
            "the final argument needs its terminator too, got {text:?}"
        );
    }

    #[test]
    fn arguments_containing_spaces_stay_on_one_line() {
        // A path with a space must not be split; the line boundary is the only
        // separator, so no quoting is applied or needed.
        let args = owned(&["-o", "/tmp/a directory/out.rmeta"]);
        let text = render(&args).expect("spaces are legal in a response file");
        assert_eq!(
            text.lines().collect::<Vec<_>>(),
            ["-o", "/tmp/a directory/out.rmeta"]
        );
    }

    #[test]
    fn a_newline_in_an_argument_is_rejected() {
        let error = render(&owned(&["-L", "dependency=/tmp/a\nb"]))
            .expect_err("a newline would silently become an argument boundary");
        assert!(
            error.to_string().contains("newline"),
            "the error should name the cause, got {error}"
        );
    }

    #[test]
    fn an_empty_argument_list_renders_empty() {
        assert_eq!(render(&[]).expect("no arguments render"), "");
    }

    proptest! {
        /// Preserve every newline-free compiler argument in ordered UTF-8 form.
        #[test]
        fn render_preserves_newline_free_argument_vectors(
            args in proptest::collection::vec("[\\p{L}\\p{N}\\p{P}\\p{Zs}]{0,32}", 0..32),
        ) {
            let expected = args.iter().fold(String::new(), |mut text, argument| {
                text.push_str(argument);
                text.push('\n');
                text
            });
            let actual = render(&args).map_err(|error| error.to_string());

            prop_assert_eq!(actual, Ok(expected));
        }
    }

    #[test]
    fn the_written_file_retains_every_compiler_argument() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let args = owned(&[
            "--edition=2024",
            "--crate-type=bin",
            "--emit=metadata",
            "/repo/tests/ui/fixture.rs",
            "--extern",
            "test_support=/target/debug/libtest_support.rmeta",
            "-L",
            "dependency=/target/debug/build/anyhow/1/out",
            "-L",
            "dependency=/target/debug/build/serde/2/out",
            "-o",
            "/tmp/out.rmeta",
        ]);
        let arg = write(dir.path(), "ui.args", &args).expect("write the response file");

        let path = arg
            .strip_prefix('@')
            .expect("the argument passed to rustc is @<path>");
        let text = test_support::fs::read_to_string(path).expect("read the response file back");
        assert_eq!(
            text.lines().collect::<Vec<_>>(),
            args,
            "every argument should survive the round trip, in order"
        );
        // Spot-check the categories the harnesses depend on, so a future
        // refactor that drops one fails here rather than as an E0463 or a
        // missing-output error from rustc.
        assert!(
            text.contains("\n/repo/tests/ui/fixture.rs\n"),
            "source path retained"
        );
        assert!(text.contains("\n--extern\n"), "extern flag retained");
        assert_eq!(
            text.matches("\ndependency=").count(),
            2,
            "every dependency search directory retained"
        );
        assert!(
            text.ends_with("-o\n/tmp/out.rmeta\n"),
            "output path retained last"
        );
    }
}
