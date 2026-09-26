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
///
/// Deliberately carries no `parse`/`as_str`/`ALL` table yet. Those exist to
/// serve the recipe-text filters' `dialect` keyword argument, and they arrive
/// with that consumer in EP-M4; adding them here would be dead code no profile
/// could attribute correctly, since the unit tests below would make a
/// `dead_code` expectation unfulfilled exactly in the `--all-targets` profile
/// the gates use.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ShellDialect {
    /// POSIX `sh` word quoting, also correct for Bash and Z Shell.
    Sh,
    /// Windows PowerShell single-quoted string quoting.
    PowerShell,
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
///
/// This call site is *not* yet marked with a `clippy::disallowed_methods`
/// expectation, because `clippy.toml` does not disallow `QuoteRefExt::quoted`
/// until EP-M4 adds that entry alongside the second expectation in
/// `src/stdlib/command/quote.rs`. Adding the entry in EP-M3 would immediately
/// make that second site a violation and force an edit to a file this
/// milestone's conformance check requires to stay untouched, so the entry, both
/// expectations, and the call sites they sanction land together.
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
