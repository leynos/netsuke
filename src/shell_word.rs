//! Encode one string as a single shell word for a named dialect.
//!
//! This is a leaf below IR lowering, Ninja rendering, and the standard
//! library: all three may depend on it, and it depends on nothing above them.
//! It exists so that exactly one recipe-shell quoting implementation is
//! compiled in — `quote_path` and the recipe-text template filters both
//! delegate here instead of each carrying their own encoder.
//!
//! It deliberately does **not** live beside [`crate::recipe_shell`]. That
//! module is the data-only vocabulary type three layers agree on, and giving
//! it a `shell_quote` dependency would change its character. The dependency
//! runs the other way: `RecipeShell` knows its dialect, and this module never
//! names `RecipeShell`.

use shell_quote::{QuoteRefExt, Sh};

/// The shell dialect a word is encoded for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ShellDialect {
    /// POSIX `sh` word quoting, also correct for Bash and Z Shell.
    Sh,
    /// Windows PowerShell single-quoted string quoting.
    PowerShell,
}

impl ShellDialect {
    /// Every dialect, in the order errors enumerate them.
    pub(crate) const ALL: &'static [Self] = &[Self::Sh, Self::PowerShell];

    /// Return the `dialect` keyword-argument spelling of this dialect.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Sh => "sh",
            Self::PowerShell => "powershell",
        }
    }

    /// Return the dialect's name as a metric label.
    ///
    /// A `'static` value that is one of the closed set a recorder admits, so a
    /// counter series stays bounded. Distinct from [`Self::as_str`] only in
    /// intent: that one is the spelling a manifest writes and may gain
    /// synonyms, this one must not.
    pub(crate) const fn telemetry_name(self) -> &'static str {
        self.as_str()
    }

    /// Parse one `dialect` keyword argument, case-insensitively.
    ///
    /// Deliberately rejects `bash`. See decision D3: `RecipeShell::Bash` maps
    /// to `Sh` because `Sh` output is valid Bash, but the `shell-quote` crate's
    /// `Bash` encoder emits a different `$'...'` form that Netsuke does not
    /// compile in. Accepting the name now would lock in a meaning a real
    /// `bash` dialect would have to break.
    pub(crate) fn parse(raw: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .copied()
            .find(|dialect| raw.eq_ignore_ascii_case(dialect.as_str()))
    }
}

/// Report whether `value` can survive as part of a single-line recipe.
///
/// Newline, carriage return, and NUL cannot: a Ninja binding is single-line by
/// construction. This is the same rule the Ninja writer enforces at its own
/// boundary; see ADR-014. It is promoted here so one predicate has one
/// definition rather than a copy at each enforcement point.
pub(crate) fn is_recipe_admissible(value: &str) -> bool {
    !value.contains(['\n', '\r', '\0'])
}

/// Encode `value` as one shell word for `dialect`.
///
/// This function is total: the caller owns the question of which inputs a
/// recipe may carry, and asks [`is_recipe_admissible`] separately. `quote_path`
/// depends on that split to keep its existing total behaviour.
///
/// The `Sh` arm delegates to `shell_quote`'s quoter, which is *suffix*-quoting
/// rather than canonically enclosing: it leaves the longest safe prefix bare and
/// quotes only the remainder, so `a b` becomes `a' b'` and not `'a b'`. The
/// contract is therefore round-tripping, not any particular shape — every output
/// decodes back to `value` under a POSIX shell. The table in
/// `sh_quoting_is_minimal` pins the shapes this version actually produces.
#[expect(
    clippy::disallowed_methods,
    reason = "The one sanctioned implementation of recipe-shell word quoting; \
              shell_word exists so every other call site delegates here."
)]
pub(crate) fn quote_word(dialect: ShellDialect, value: &str) -> String {
    if dialect == ShellDialect::PowerShell {
        return format!("'{}'", value.replace('\'', "''"));
    }
    // `shell_quote` returns bytes because a `Vec<u8>` argument need not be
    // UTF-8; a `&str` always is, so this conversion cannot lose information.
    let bytes: Vec<u8> = value.quoted(Sh);
    match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(err) => {
            debug_assert!(false, "shell quoting produced non UTF-8 bytes: {err}");
            String::from_utf8_lossy(err.as_bytes()).into_owned()
        }
    }
}

#[cfg(test)]
mod tests {
    //! Tests for the dialect mapping, the admissibility rule, and encoding.
    use super::{ShellDialect, is_recipe_admissible, quote_word};
    use crate::recipe_shell::RecipeShell;
    use rstest::rstest;

    /// `Posix` and `Bash` differ in transport, not in lexis, so both map to
    /// `Sh`; only `PowerShell` gets the PowerShell encoder.
    #[rstest]
    #[case::posix(RecipeShell::Posix, ShellDialect::Sh)]
    #[case::bash(RecipeShell::Bash, ShellDialect::Sh)]
    #[case::power_shell(RecipeShell::PowerShell, ShellDialect::PowerShell)]
    fn every_recipe_shell_has_one_dialect(
        #[case] shell: RecipeShell,
        #[case] expected: ShellDialect,
    ) {
        assert_eq!(shell.dialect(), expected);
    }

    /// Every dialect names itself, and `ALL` enumerates each exactly once.
    ///
    /// `ALL` is what the `dialect_invalid` diagnostic renders, so a dialect
    /// missing from it would be unnameable and unparseable at once.
    #[rstest]
    fn dialect_names_are_unique_and_complete() {
        let names = ShellDialect::ALL
            .iter()
            .map(|dialect| dialect.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["sh", "powershell"]);
        let unique: std::collections::BTreeSet<_> = names.iter().collect();
        assert_eq!(
            unique.len(),
            names.len(),
            "every dialect must have a distinct spelling: {names:?}"
        );
    }

    /// Every name in `ALL` parses, case-insensitively, and round-trips.
    #[rstest]
    fn every_dialect_name_parses(
        #[values(ShellDialect::Sh, ShellDialect::PowerShell)] dialect: ShellDialect,
    ) {
        let name = dialect.as_str();
        assert_eq!(ShellDialect::parse(name), Some(dialect));
        assert_eq!(ShellDialect::parse(&name.to_uppercase()), Some(dialect));
    }

    /// The telemetry name is exactly the spelling a manifest writes.
    ///
    /// [`ShellDialect::telemetry_name`] exists so the metric label vocabulary
    /// is a decision separate from the keyword-argument spelling. This pins the
    /// two together today, so the day a synonym is added to `as_str` the test
    /// asks whether the label set should widen too, rather than letting the
    /// counter emit a value the recorder does not admit. That failure is
    /// silent: an unadmitted series returns a noop handle and records nothing.
    #[rstest]
    fn the_telemetry_name_is_the_manifest_spelling() {
        for dialect in ShellDialect::ALL {
            assert_eq!(
                dialect.telemetry_name(),
                dialect.as_str(),
                "telemetry_name must name the same dialect as_str does"
            );
        }
    }

    /// Names outside `ALL` are rejected rather than guessed at.
    ///
    /// `bash` is the case that matters: D3 declines it deliberately, because
    /// `shell-quote`'s `Bash` encoder emits a `$'...'` form Netsuke does not
    /// compile in. `zsh` and `sh`-adjacent spellings are near misses that a
    /// prefix or alias match would wrongly accept.
    #[rstest]
    #[case::bash("bash")]
    #[case::cmd("cmd")]
    #[case::zsh("zsh")]
    #[case::pwsh("pwsh")]
    #[case::powershell_exe("powershell.exe")]
    #[case::empty("")]
    #[case::whitespace(" sh ")]
    fn unknown_dialect_names_are_rejected(#[case] raw: &str) {
        assert_eq!(ShellDialect::parse(raw), None, "parse({raw:?})");
    }

    /// The admissibility rule rejects exactly the three control characters.
    #[rstest]
    #[case::plain("plain", true)]
    #[case::empty("", true)]
    #[case::space("a b", true)]
    #[case::quote("it's", true)]
    #[case::tab("a\tb", true)]
    #[case::newline("a\nb", false)]
    #[case::carriage_return("a\rb", false)]
    #[case::nul("a\0b", false)]
    fn admissibility_rejects_only_control_characters(#[case] value: &str, #[case] expected: bool) {
        assert_eq!(
            is_recipe_admissible(value),
            expected,
            "is_recipe_admissible({value:?})"
        );
    }

    /// `Sh` output is the minimal form: bare where safe, quoted where not.
    ///
    /// The expected values are **measured**, not derived — they pin the current
    /// output of `shell-quote` 0.7.2 with `default-features = false,
    /// features = ["sh"]`, and that encoder emits a *suffix-quoting* form
    /// rather than a canonically enclosing one: it leaves the safe prefix bare
    /// and quotes only the remainder, so `a b` becomes `a' b'` and not `'a b'`.
    /// Each of these decodes back to its input — `it\'s` is `it` + `\'` + `s` —
    /// which is the property that actually matters and which
    /// `OBL-SH-ROUNDTRIP` discharges against a real `/bin/sh` in EP-M4.
    ///
    /// This table exists as a change detector: if a dependency bump alters the
    /// encoder's style, the Ninja snapshot suite changes too, and this test
    /// localizes the cause to the encoder rather than to recipe lowering.
    #[rstest]
    #[case::bare("plain", "plain")]
    #[case::space("a b", r"a' b'")]
    #[case::single_quote("it's", r"it\'s")]
    fn sh_quoting_is_minimal(#[case] value: &str, #[case] expected: &str) {
        assert_eq!(quote_word(ShellDialect::Sh, value), expected);
    }

    /// PowerShell doubles an embedded quote and never escapes a backtick.
    #[rstest]
    #[case::bare("plain", "'plain'")]
    #[case::space("a b", "'a b'")]
    #[case::single_quote("it's", "'it''s'")]
    #[case::dollar("$HOME", "'$HOME'")]
    #[case::empty("", "''")]
    fn power_shell_quoting_doubles_quotes(#[case] value: &str, #[case] expected: &str) {
        assert_eq!(quote_word(ShellDialect::PowerShell, value), expected);
    }
}
