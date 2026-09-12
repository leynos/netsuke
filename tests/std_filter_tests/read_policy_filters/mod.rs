//! File-reading policy tests for the bounded `contents`, `linecount`, `hash`,
//! and `digest` filters.
//!
//! The file-type and budget clauses of that policy are shared by all four
//! filters, so they are asserted through one table of filter cases: a policy
//! enforced on only some entry points then fails on the entry point that
//! ignores it instead of passing because a sibling filter was tested. This
//! module owns the table and the render helpers; `budget_tests` pins the byte
//! budget, and `file_type_tests` pins the symlink and file-type policy.
use anyhow::{Context, Result, anyhow, bail, ensure};
use camino::Utf8Path;
use cap_std::{ambient_authority, fs_utf8::Dir};
use minijinja::context;

#[cfg(unix)]
use rustix::fs::{Dev, FileType as RxFileType, Mode, mknodat};

pub(super) use super::support::fallible;

/// Inputs for one bounded-read render against a policy workspace.
#[derive(Clone, Copy)]
pub(super) struct PolicyRender<'a> {
    /// File-read budget configured on the stdlib environment.
    pub(super) limit: u64,
    /// Template registration name.
    pub(super) name: &'a str,
    /// Template source exercising a file-reading filter.
    pub(super) template: &'a str,
    /// Workspace root the stdlib environment is bound to.
    pub(super) root: &'a camino::Utf8Path,
    /// Path the template reads.
    pub(super) path: &'a camino::Utf8Path,
}

/// One file-reading filter, addressed through the template call it registers.
#[derive(Clone, Copy, Debug)]
pub(super) struct FilterCase {
    /// Registered filter name, which also names each rendered template.
    pub(super) name: &'static str,
    /// Positional arguments the filter requires, without the parentheses.
    pub(super) positional: &'static str,
    /// Expected render for the `file` fixture (contents `data`) when the
    /// caller opts into following a symlink to it.
    pub(super) followed: &'static str,
    /// Expected render for the `exact.bin` fixture (`12345`) read at exactly
    /// its own length.
    pub(super) exact: &'static str,
}

impl FilterCase {
    /// The template that reads `path` through this filter.
    pub(super) fn template(self) -> String {
        self.render(self.positional)
    }

    /// The template that reads `path` through this filter with `kwargs`.
    pub(super) fn template_with(self, kwargs: &str) -> String {
        let arguments = if self.positional.is_empty() {
            kwargs.to_owned()
        } else {
            format!("{}, {kwargs}", self.positional)
        };
        self.render(&arguments)
    }

    /// Wrap `arguments` in a template that pipes `path` through this filter.
    fn render(self, arguments: &str) -> String {
        if arguments.is_empty() {
            format!("{{{{ path | {} }}}}", self.name)
        } else {
            format!("{{{{ path | {}({arguments}) }}}}", self.name)
        }
    }

    /// A template name unique to this filter and `suffix`.
    fn name_for(self, suffix: &str) -> String {
        format!("{}_{suffix}", self.name)
    }
}

/// The `contents` filter, which renders UTF-8 text.
pub(super) const CONTENTS: FilterCase = FilterCase {
    name: "contents",
    positional: "",
    followed: "data",
    exact: "12345",
};

/// The `linecount` filter, which counts lines in UTF-8 text.
pub(super) const LINECOUNT: FilterCase = FilterCase {
    name: "linecount",
    positional: "",
    followed: "1",
    exact: "1",
};

/// The `hash` filter, which hashes raw bytes with the default algorithm.
pub(super) const HASH: FilterCase = FilterCase {
    name: "hash",
    positional: "'sha256'",
    followed: "3a6eb0790f39ac87c94f3856b2dd2c5d110e6811602261a9a923d3bb23adc8b7",
    exact: "5994471abb01112afcc18159f6cc74b4f511b99806da59b3caf5a9c173cacfc5",
};

/// The `digest` filter, which renders the requested prefix of a hash.
pub(super) const DIGEST: FilterCase = FilterCase {
    name: "digest",
    positional: "8, 'sha256'",
    followed: "3a6eb079",
    exact: "5994471a",
};

/// Assert that `link` really is a symlink, failing setup when it is not.
///
/// The metadata read does not follow the link, so a regular file sitting at the
/// same path cannot pass for one. A special-file policy test must create the
/// requested file type or skip because that file type is unavailable; it must
/// not substitute a regular file, which here would invert the assertion — the
/// filters would be expected to reject a perfectly ordinary file.
pub(super) fn require_real_symlink(root: &Utf8Path, link: &Utf8Path) -> Result<()> {
    let dir = Dir::open_ambient_dir(root, ambient_authority())
        .with_context(|| format!("open workspace root {root} to stat the symlink fixture"))?;
    let name = link
        .file_name()
        .with_context(|| format!("symlink fixture {link} has no file name"))?;
    let metadata = dir
        .symlink_metadata(Utf8Path::new(name))
        .with_context(|| format!("stat symlink fixture {link}"))?;
    ensure!(
        metadata.file_type().is_symlink(),
        "fixture {link} is not a symlink; the symlink policy cannot be exercised \
         without one, and substituting a regular file would invert the assertions"
    );
    Ok(())
}

/// Write `contents` to `name` inside `root`, returning the fixture's path.
pub(super) fn write_fixture(
    root: &Utf8Path,
    name: &str,
    contents: &[u8],
) -> Result<camino::Utf8PathBuf> {
    let dir = Dir::open_ambient_dir(root, ambient_authority())
        .with_context(|| format!("open workspace root {root} to write {name}"))?;
    dir.write(name, contents)
        .with_context(|| format!("write fixture {name}"))?;
    Ok(root.join(name))
}

/// Create a FIFO named `name` inside `root`, returning its path.
///
/// A FIFO is the special file that makes the open policy observable: opening
/// one for reading blocks until a writer appears unless the open carries
/// `O_NONBLOCK`, so a policy that omits the flag wedges the calling thread
/// rather than returning a rejection.
#[cfg(unix)]
pub(super) fn create_fifo(root: &Utf8Path, name: &str) -> Result<camino::Utf8PathBuf> {
    let dir = Dir::open_ambient_dir(root, ambient_authority())
        .with_context(|| format!("open workspace root {root} to create the FIFO fixture"))?;
    mknodat(
        &dir,
        name,
        RxFileType::Fifo,
        Mode::RUSR | Mode::WUSR,
        Dev::default(),
    )
    .map_err(|err| anyhow!("create FIFO fixture {name}: {err}"))?;
    Ok(root.join(name))
}

/// Render a bounded-read template, returning the raw result for assertions.
pub(super) fn render_with_file_read_limit(
    render: PolicyRender<'_>,
) -> Result<std::result::Result<String, minijinja::Error>> {
    let mut env = fallible::stdlib_env_with_root_and_file_read_limit(render.root, render.limit)?;
    fallible::register_template(&mut env, render.name, render.template)?;
    let registered = env
        .get_template(render.name)
        .context("fetch policy template")?;
    Ok(registered.render(context!(path => render.path.as_str())))
}

/// A fixture to read and the operator ceiling to read it under.
#[derive(Clone, Copy)]
pub(super) struct ReadTarget<'a> {
    /// Workspace root the stdlib environment is bound to.
    pub(super) root: &'a Utf8Path,
    /// Path the template reads.
    pub(super) path: &'a Utf8Path,
    /// File-read budget configured on the stdlib environment.
    pub(super) limit: u64,
}

impl<'a> ReadTarget<'a> {
    /// Read `path` within `root` under a `limit`-byte budget.
    pub(super) const fn new(root: &'a Utf8Path, path: &'a Utf8Path, limit: u64) -> Self {
        Self { root, path, limit }
    }
}

/// Render `template` through `case` against `target`, tagged with `suffix`.
pub(super) fn render_case(
    case: FilterCase,
    suffix: &str,
    target: ReadTarget<'_>,
    template: &str,
) -> Result<std::result::Result<String, minijinja::Error>> {
    let name = case.name_for(suffix);
    render_with_file_read_limit(PolicyRender {
        limit: target.limit,
        name: &name,
        template,
        root: target.root,
        path: target.path,
    })
}

/// Unwrap the rejection from a render, naming the filter on failure.
pub(super) fn rejection(
    case: FilterCase,
    result: std::result::Result<String, minijinja::Error>,
) -> Result<minijinja::Error> {
    match result {
        Ok(output) => bail!("{}: expected a rejection but rendered {output}", case.name),
        Err(err) => Ok(err),
    }
}

mod budget_tests;

mod file_type_tests;
