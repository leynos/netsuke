//! Carry a borrowed target list through runner dispatch.
//!
//! The list arrives from command-line parsing — either from `--targets` or from
//! the configured defaults — and has to reach the Ninja invocation several
//! layers down. It is borrowed rather than cloned because nothing on that path
//! owns it, and empty means "whatever the IR declares", which is why the
//! wrapper exists instead of a bare `&[String]`: the difference between "no
//! targets named" and "targets named" is the type's whole job, and a slice
//! alone would leave every call site to re-derive it.

/// Target names passed through to Ninja; an empty slice uses IR defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildTargets<'a>(&'a [String]);

impl<'a> BuildTargets<'a> {
    /// Wrap a borrowed list of command-line target names.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::runner::BuildTargets;
    ///
    /// let names = vec![String::from("lint")];
    /// assert_eq!(BuildTargets::new(&names).as_slice(), names.as_slice());
    /// ```
    #[must_use]
    pub const fn new(targets: &'a [String]) -> Self {
        Self(targets)
    }

    /// Return the borrowed target names.
    ///
    /// # Examples
    ///
    /// ```
    /// use netsuke::runner::BuildTargets;
    ///
    /// let names = vec![String::from("lint")];
    /// assert_eq!(BuildTargets::new(&names).as_slice(), [String::from("lint")]);
    /// ```
    #[must_use]
    pub const fn as_slice(&self) -> &'a [String] {
        self.0
    }
}

#[expect(
    clippy::derivable_impls,
    reason = "Default derive requires 'static lifetime; manual impl returns empty slice."
)]
impl Default for BuildTargets<'_> {
    fn default() -> Self {
        Self(&[])
    }
}
