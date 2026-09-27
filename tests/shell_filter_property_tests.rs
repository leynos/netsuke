//! Property and contract tests for the `shell_quote` and `shell_join` filters.
//!
//! Each obligation in the plan has one property that states it over a generated
//! domain and, beside it, a control that shows the property can fail. A test
//! that only ever passes is not evidence, so most of the work here is arranging
//! for the fixtures to be wrong in a way the assertion must notice: a broken
//! quoter through the real harness, a POSIX-quoted string through the PowerShell
//! decoder, an auto-escaping environment through the verbatim check.
//!
//! The generated domain is a deliberate alphabet rather than `any::<String>()`.
//! Uniform bytes are almost all inert — a random string of printable ASCII is
//! mostly alphanumerics — so the interesting inputs, the ones made only of
//! metacharacters, would be vanishingly rare and the property would pass on a
//! sample that never exercised quoting at all.

use anyhow::{Context, Result, bail, ensure};
use camino::Utf8PathBuf;
use minijinja::{AutoEscape, Environment, context, value::Value};
use netsuke::manifest::{self, EnvReader, ManifestEnvironment};
use netsuke::stdlib::{NetworkPolicy, StdlibConfig};
use proptest::prelude::*;
use proptest::test_runner::{FileFailurePersistence, TestRunner};
use rstest::rstest;
use std::cell::Cell;
use std::process::Command;

/// The dialect names the filters accept, spelled as call sites must spell them.
const SH: &str = "sh";
/// The PowerShell dialect name.
const POWERSHELL: &str = "powershell";

/// Characters the shell treats as special, plus enough letters to build words.
///
/// Every entry earns its place: the quotes and backslash are the escaping
/// cases, the expansion characters (`$`, backtick) are the ones a wrong encoder
/// silently expands, the glob and grouping characters (`*`, `?`, `[`, `{`) are
/// the ones that change the *shape* of an argument vector, and the separators
/// (`;`, `&`, `|`, newline-adjacent control characters) are the ones that split
/// one word into several. Tab is here because it is both a shell separator and
/// a control character the encoder must carry literally.
const ALPHABET: &[char] = &[
    'a', 'b', 'z', 'A', 'Z', '0', '9', '_', '-', '.', '/', ',', ' ', '\t', '\'', '"', '$', '`',
    '\\', '*', '?', ';', '&', '|', '<', '>', '(', ')', '[', ']', '{', '}', '#', '~', '!', '=', ':',
    '@', '%', '^', '+', 'é', '中', '\u{80}', '\u{a0}',
];

/// The expansion-and-quote witness the plan names as its worst case.
///
/// `shell_quote` must not leave the expansion live, and must not leave the
/// quote unbalanced. Kept as a constant so the corpus-span check below can
/// require it, rather than hoping a random draw produces it.
const EXPANSION_WITNESS: &str = "$HOME 'x'";

/// A string drawn from [`ALPHABET`], admissible as a single-line recipe word.
///
/// Lengths run to twenty-four characters, which is long enough to reach several
/// quoting transitions — the encoder alternates in and out of quotes per run.
///
/// The empty word and [`EXPANSION_WITNESS`] are seeded into one arm each. Both
/// are reachable by chance — an empty draw is one of the twenty-five lengths,
/// and the witness is one of astronomically many draws — but a corpus-span
/// check that requires them would then fail on an unlucky seed roughly one run
/// in fourteen, which is a flaky control rather than a control.
fn word() -> impl Strategy<Value = String> {
    prop_oneof![
        1 => Just(String::new()),
        1 => Just(EXPANSION_WITNESS.to_owned()),
        8 => prop::collection::vec(prop::sample::select(ALPHABET), 0..24).prop_map(|chars| {
            chars
                .into_iter()
                .filter(|character| !matches!(character, '\n' | '\r' | '\0'))
                .collect()
        }),
    ]
}

/// A string that is never empty, for cases where the empty word is not the
/// subject under test and would only add a degenerate case.
fn non_empty_word() -> impl Strategy<Value = String> {
    word().prop_filter("the word must not be empty", |value| !value.is_empty())
}

/// Locate `sh`, or `None` when the host has no POSIX shell.
///
/// Homebrew's macOS `sh` lives outside the default `PATH` some CI runners set,
/// so the well-known absolute paths are tried after the `PATH` lookup.
fn posix_shell() -> Option<Utf8PathBuf> {
    for candidate in ["/bin/sh", "/usr/bin/sh", "/usr/local/bin/sh"] {
        let path = Utf8PathBuf::from(candidate);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

/// Run `script` under a real POSIX shell and return its stdout.
fn run_posix_shell(shell: &Utf8PathBuf, script: &str) -> Result<String> {
    let output = Command::new(shell.as_str())
        .arg("-c")
        .arg(script)
        .output()
        .with_context(|| format!("run {shell} -c {script}"))?;
    ensure!(
        output.status.success(),
        "{shell} exited with {} for {script}: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).context("shell stdout should be valid UTF-8")
}

/// The rendered text of `template` under a stdlib environment with `dialect`.
///
/// The environment is registered through `stdlib::register_with_config`, so the
/// filters under test are the ones a manifest actually receives rather than a
/// re-registration of the same closures.
///
/// `subject` is bound to the template variables `value` and `values` rather
/// than spliced into the source. Binding under both names lets one helper serve
/// the scalar and the list filter; interpolating a Rust `{:?}` rendering would
/// work for the common case and break on exactly the inputs this file exists to
/// exercise — a word containing `\u{80}` or a backslash is not a valid Jinja
/// string escape, and the resulting diagnostic would be an escaping bug in the
/// test harness masquerading as one in the filter.
fn render_with(template: &str, dialect: &str, subject: &Value) -> Result<String> {
    let config = StdlibConfig::from_current_dir()?;
    let config = match dialect {
        SH => config.with_recipe_shell(netsuke::recipe_shell::RecipeShell::Posix),
        POWERSHELL => config.with_recipe_shell(netsuke::recipe_shell::RecipeShell::PowerShell),
        other => bail!("unknown test dialect {other}"),
    };
    let mut env = Environment::new();
    netsuke::stdlib::register_with_config(&mut env, config)?;
    // The `?` converts `minijinja::Error` into the `anyhow::Error` this helper
    // returns, so it is load-bearing rather than a `needless_question_mark`.
    Ok(env.render_str(template, context! { value => subject, values => subject })?)
}

/// Render `{{ value | shell_quote }}` with an omitted dialect, for `dialect`.
fn quote_value(value: &str, dialect: &str) -> Result<String> {
    render_with("{{ value | shell_quote }}", dialect, &Value::from(value))
}

/// Decode one PowerShell single-quoted literal written by the encoder.
///
/// This is a *decoder*, not a restatement of the encoder: it strips the
/// enclosing quotes and collapses each doubled quote. Writing it the other way
/// round — calling the encoder and comparing — would prove only that the
/// function is a function.
///
/// # Errors
///
/// Returns an error if `text` is not a single-quoted PowerShell literal.
fn decode_power_shell_literal(text: &str) -> Result<String> {
    let inner = text
        .strip_prefix('\'')
        .and_then(|rest| rest.strip_suffix('\''))
        .with_context(|| format!("not a PowerShell single-quoted literal: {text:?}"))?;
    Ok(inner.replace("''", "'"))
}

/// The bytes a POSIX shell reads back, given the encoder's output.
///
/// `printf %s` writes its argument without a trailing newline, so the child's
/// stdout is the decoded word and nothing else. Passing the encoded text as the
/// script's argument rather than splicing it into the script keeps the harness
/// honest: the shell parses the word from argument position, which is exactly
/// where a recipe's word sits.
fn decode_through_posix_shell(shell: &Utf8PathBuf, encoded: &str) -> Result<String> {
    let script = format!(r#"printf %s {encoded}"#);
    run_posix_shell(shell, &script)
}

/// Split `text` as a POSIX shell would, using the same lexer the IR uses.
fn split(text: &str) -> Result<Vec<String>> {
    shlex::split(text).with_context(|| format!("shlex could not split {text:?}"))
}

/// The POSIX encoder's output for `value`, read out of the filter itself.
fn encoded_sh(value: &str) -> Result<String> {
    quote_value(value, SH)
}

// ---------------------------------------------------------------------------
// OBL-SH-ROUNDTRIP
// ---------------------------------------------------------------------------

proptest! {
    // 64 cases, not 128: every case forks a shell, and the whole file must
    // finish inside the 10s budget the milestones set for it. The corpus is
    // still wide enough to reach the boundary — the companion test below
    // asserts that it does rather than assuming it.
    #![proptest_config(ProptestConfig {
        cases: 64,
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(
            "tests/shell_filter_property_tests.proptest-regressions",
        ))),
        ..ProptestConfig::default()
    })]

    /// A real shell reads back exactly what was quoted.
    #[test]
    fn sh_quoting_round_trips_through_a_real_shell(value in word()) {
        let Some(shell) = posix_shell() else {
            // No POSIX shell on this host: the obligation is undischarged here
            // and the `#[cfg(unix)]` companion below says so explicitly. Failing
            // rather than passing keeps the gap visible — a silent return would
            // report green for a property that never ran.
            return Err(TestCaseError::fail("no POSIX shell available"));
        };
        let encoded = encoded_sh(&value)
            .map_err(|error| TestCaseError::fail(format!("{error:#}")))?;
        let decoded = decode_through_posix_shell(&shell, &encoded)
            .map_err(|error| TestCaseError::fail(format!("{error:#}")))?;
        prop_assert_eq!(&decoded, &value, "quoted {:?} as {:?}", value, encoded);
    }
}

/// The subprocess harness rejects a quoter that is wrong.
///
/// The witness is the plan's own worst case: `$HOME 'x'` contains an expansion,
/// a space, and a single quote. Wrapping it in double quotes — the naive fix a
/// manifest author reaches for — leaves the expansion live and the quote
/// unbalanced, so a real shell does not return the input. If this ever passes,
/// [`decode_through_posix_shell`] is not measuring anything and every green case
/// above is void.
#[cfg(unix)]
#[rstest]
fn the_posix_harness_rejects_a_naive_quoter() -> Result<()> {
    let shell = posix_shell().context("a POSIX shell is required for this control")?;
    let witness = "$HOME 'x'";
    let naive = format!("\"{witness}\"");
    let decoded = decode_through_posix_shell(&shell, &naive)?;
    assert_ne!(
        decoded, witness,
        "the harness accepted a naive double-quoting; it is not measuring quoting"
    );
    Ok(())
}

/// The generated corpus reaches the quoting boundary, not just alphanumerics.
///
/// The property above can only fail on inputs that need quoting. Without this,
/// a generator that happened to produce nothing but inert characters would let
/// the property pass while testing nothing. The tallies live in a `RefCell`
/// because `TestRunner::run` takes an `Fn`.
#[test]
fn the_generated_corpus_spans_the_quoting_boundary() {
    let tallies: Cell<Corpus> = Cell::new(Corpus::default());
    TestRunner::new(ProptestConfig {
        cases: 64,
        ..ProptestConfig::default()
    })
    .run(&word(), |value| {
        tallies.set(tallies.get().extended(&value));
        Ok(())
    })
    .expect("the word corpus should be generatable");

    let observed = tallies.into_inner();
    assert!(
        observed.with_quote > 0
            && observed.with_dollar > 0
            && observed.with_space > 0
            && observed.with_control > 0
            && observed.empty > 0
            && observed.quoted > 0,
        "the corpus must reach every quoting boundary: {observed:?}"
    );
}

/// Tallies of the quoting boundaries a generated corpus reached.
#[derive(Clone, Copy, Debug, Default)]
struct Corpus {
    /// Words containing a single quote, the escaping case.
    with_quote: usize,
    /// Words containing a dollar sign, the expansion case.
    with_dollar: usize,
    /// Words containing a space, the splitting case.
    with_space: usize,
    /// Words containing a C0 control character other than the three rejected.
    with_control: usize,
    /// The empty word, which the encoder must render as `''`.
    empty: usize,
    /// Words the encoder did not leave bare, i.e. cases that exercised quoting.
    quoted: usize,
}

impl Corpus {
    /// Fold one generated word into the tallies, returning the result.
    ///
    /// `Cell::get` needs `Copy` and `Cell::set` needs a whole value, so the
    /// tally is a fold rather than an in-place mutation.
    fn extended(self, value: &str) -> Self {
        // "Did this need quoting" is asked of the encoder, through the same
        // filter the property exercises, rather than of a re-derived character
        // class that could disagree with it.
        let needed_quoting = encoded_sh(value).is_ok_and(|encoded| encoded != value);
        Self {
            with_quote: self.with_quote + usize::from(value.contains('\'')),
            with_dollar: self.with_dollar + usize::from(value.contains('$')),
            with_space: self.with_space + usize::from(value.contains(' ')),
            with_control: self.with_control
                + usize::from(
                    value.chars().any(|character| {
                        character.is_control() && !matches!(character, '\n' | '\r')
                    }),
                ),
            empty: self.empty + usize::from(value.is_empty()),
            quoted: self.quoted + usize::from(needed_quoting),
        }
    }
}

// ---------------------------------------------------------------------------
// OBL-PS-ROUNDTRIP
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 128,
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(
            "tests/shell_filter_property_tests.proptest-regressions",
        ))),
        ..ProptestConfig::default()
    })]

    /// The PowerShell model decodes back to the input for every word.
    #[test]
    fn power_shell_quoting_round_trips_through_the_model(value in word()) {
        let encoded = quote_value(&value, POWERSHELL)
            .map_err(|error| TestCaseError::fail(format!("{error:#}")))?;
        let decoded = decode_power_shell_literal(&encoded)
            .map_err(|error| TestCaseError::fail(format!("{error:#}")))?;
        prop_assert_eq!(&decoded, &value, "quoted {:?} as {:?}", value, encoded);
    }
}

/// The PowerShell decoder rejects POSIX quoting.
///
/// Without this, a decoder that returned its input unchanged — or one that
/// merely stripped the first and last byte — would satisfy the property above
/// for every word the POSIX encoder already leaves bare. The witness is chosen
/// so the two encoders genuinely differ: POSIX suffix-quoting emits `a' b'`,
/// which has no enclosing quotes to strip.
#[test]
fn the_power_shell_decoder_rejects_posix_quoting() -> Result<()> {
    let posix = encoded_sh("a b")?;
    assert_eq!(posix, "a' b'", "POSIX quoting changed shape");
    let decoded = decode_power_shell_literal(&posix);
    ensure!(
        decoded.is_err(),
        "the decoder accepted POSIX output {posix:?} as a PowerShell literal"
    );
    Ok(())
}

/// The name of an interpreter to try, in the order a host is likely to have it.
///
/// `powershell.exe` is the Windows PowerShell the recipes actually run under;
/// `pwsh` is PowerShell Core, which parses single-quoted strings identically
/// and so can discharge the same axiom on a non-Windows CI host.
const POWER_SHELL_CANDIDATES: &[&str] = &["powershell.exe", "pwsh", "powershell"];

/// Run `script` under the first PowerShell interpreter this host provides.
///
/// # Errors
///
/// Returns `Ok(None)` when no interpreter is installed, or an error when one is
/// installed and fails to run.
fn run_power_shell(script: &str) -> Result<Option<String>> {
    for candidate in POWER_SHELL_CANDIDATES {
        let output = match Command::new(candidate)
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                script,
            ])
            .output()
        {
            Ok(output) => output,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(error).with_context(|| format!("run {candidate}"));
            }
        };
        ensure!(
            output.status.success(),
            "{candidate} exited with {} for {script}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout)
            .with_context(|| format!("{candidate} stdout should be valid UTF-8"))?;
        // PowerShell terminates its output with a newline the writer did not
        // ask for; the round trip is about the bytes, so it is stripped here
        // rather than papered over in the assertion.
        return Ok(Some(stdout.trim_end_matches(['\r', '\n']).to_owned()));
    }
    Ok(None)
}

/// The model agrees with the real interpreter where one is present.
///
/// On Linux the model is the only available oracle, which is why AXIOM-2 rests
/// on it there; this case discharges the gap wherever an interpreter exists
/// (Windows CI, or a host with PowerShell Core installed).
#[test]
fn the_power_shell_model_matches_the_real_interpreter() -> Result<()> {
    for value in ["a b", "it's", "$HOME", "", "a\"b", "中", "x\ty"] {
        let encoded = quote_value(value, POWERSHELL)?;
        let script = format!("[Console]::Out.Write({encoded})");
        let Some(output) = run_power_shell(&script)? else {
            eprintln!("skipped: no PowerShell interpreter on this host");
            return Ok(());
        };
        ensure!(
            output == value,
            "the interpreter read {output:?} back from {encoded} for {value:?}"
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// OBL-ONE-WORD
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 128,
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(
            "tests/shell_filter_property_tests.proptest-regressions",
        ))),
        ..ProptestConfig::default()
    })]

    /// A quoted value is exactly one shell word.
    #[test]
    fn sh_quoting_yields_exactly_one_word(value in word()) {
        let encoded = encoded_sh(&value)
            .map_err(|error| TestCaseError::fail(format!("{error:#}")))?;
        let words = split(&encoded)
            .map_err(|error| TestCaseError::fail(format!("{error:#}")))?;
        prop_assert_eq!(&words, &[value.clone()],
            "quoted {:?} as {:?}", value, encoded);
    }
}

/// The oracle distinguishes quoted from unquoted text.
///
/// `shlex::split` would satisfy the property for any encoder that happened to
/// emit a single inert word, including the identity function on single-word
/// inputs. Showing that it returns *two* words for the unquoted form is what
/// makes the one-word assertion load-bearing.
#[test]
fn the_split_oracle_sees_the_unquoted_form_as_two_words() -> Result<()> {
    let unquoted = split("a b")?;
    ensure!(
        unquoted == ["a", "b"],
        "the oracle should split an unquoted space: {unquoted:?}"
    );
    ensure!(
        split(&encoded_sh("a b")?)?.len() == 1,
        "the oracle should see the quoted form as one word"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// OBL-JOIN-SPLIT
// ---------------------------------------------------------------------------

/// Render `{{ values | shell_join }}` under the environment for `dialect`.
fn join_values(values: &[String], dialect: &str) -> Result<String> {
    let subject = Value::from_serialize(values);
    render_with("{{ values | shell_join }}", dialect, &subject)
}

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 128,
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(
            "tests/shell_filter_property_tests.proptest-regressions",
        ))),
        ..ProptestConfig::default()
    })]

    /// Joining and splitting are inverse over sequences of words.
    #[test]
    fn shell_join_inverts_word_splitting(values in prop::collection::vec(word(), 0..6)) {
        let joined = join_values(&values, SH)
            .map_err(|error| TestCaseError::fail(format!("{error:#}")))?;
        let words = split(&joined)
            .map_err(|error| TestCaseError::fail(format!("{error:#}")))?;
        prop_assert_eq!(words, values, "joined as {:?}", joined);
    }
}

/// The empty list and the list holding one empty word render differently.
///
/// These are the two boundaries a `join` implementation is most likely to
/// conflate: if the empty string were emitted for both, one of the two
/// round-trip directions would silently produce the wrong list. `shlex` reads
/// `""` as no words and `''` as one empty word, so a naive implementation that
/// rendered the second as the first would be caught here and nowhere else.
#[rstest]
fn shell_join_distinguishes_an_empty_list_from_one_empty_word() -> Result<()> {
    let empty_list = join_values(&[], SH)?;
    assert_eq!(empty_list, "", "an empty list renders as the empty string");
    assert_eq!(
        split(&empty_list)?,
        Vec::<String>::new(),
        "the empty string splits into no words"
    );

    let one_empty_word = join_values(&[String::new()], SH)?;
    assert_eq!(
        one_empty_word, "''",
        "one empty word renders as a quoted pair"
    );
    assert_eq!(
        split(&one_empty_word)?,
        vec![String::new()],
        "the quoted pair splits into one empty word"
    );
    Ok(())
}

/// The generated join corpus spans the quoting boundary.
///
/// The elements that matter are the ones a naive `join(" ")` would corrupt: a
/// member containing a space, and a member that is the empty string. Without a
/// tally this property could hold over lists of single inert words and never
/// exercise either.
#[test]
fn the_join_corpus_spans_the_quoting_boundary() {
    let tallies: Cell<(usize, usize, usize)> = Cell::new((0, 0, 0));
    TestRunner::new(ProptestConfig {
        cases: 128,
        ..ProptestConfig::default()
    })
    .run(&prop::collection::vec(word(), 0..6), |values| {
        let (spaced, empty, lists) = tallies.get();
        let has_space = values.iter().any(|value| value.contains(' '));
        let has_empty = values.iter().any(String::is_empty);
        tallies.set((
            spaced + usize::from(has_space),
            empty + usize::from(has_empty),
            lists + 1,
        ));
        Ok(())
    })
    .expect("the join corpus should be generatable");

    let (spaced, empty, lists) = tallies.into_inner();
    assert!(
        spaced > 0 && empty > 0 && lists > 0,
        "the corpus must include spaced and empty elements: \
         {spaced} spaced, {empty} empty, over {lists} lists"
    );
}

/// A naive `join(" ")` fails the property the filter satisfies.
///
/// `xs.join(" ")` looks equivalent for single words and is wrong for anything
/// with a space in it: the shell re-splits that word into two. The control is
/// what separates "the filter joins" from "the filter joins *and quotes*".
#[test]
fn a_naive_join_fails_the_split_round_trip() -> Result<()> {
    let values = vec!["a b".to_owned(), "-C".to_owned()];
    let naive = values.join(" ");
    assert_eq!(naive, "a b -C");
    assert_ne!(
        split(&naive)?,
        values,
        "the naive join should not round-trip, or the control proves nothing"
    );
    assert_eq!(
        split(&join_values(&values, SH)?)?,
        values,
        "the filter's own join should round-trip the same input"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// OBL-JOIN-QUOTE-AGREE
// ---------------------------------------------------------------------------

proptest! {
    #![proptest_config(ProptestConfig {
        cases: 128,
        failure_persistence: Some(Box::new(FileFailurePersistence::Direct(
            "tests/shell_filter_property_tests.proptest-regressions",
        ))),
        ..ProptestConfig::default()
    })]

    /// `shell_join` is `shell_quote` distributed over a list.
    #[test]
    fn shell_join_agrees_with_quoting_each_element(
        values in prop::collection::vec(non_empty_word(), 1..5),
        power_shell in any::<bool>(),
    ) {
        let dialect = if power_shell { POWERSHELL } else { SH };
        let joined = join_values(&values, dialect)
            .map_err(|error| TestCaseError::fail(format!("{error:#}")))?;
        let mut individually = Vec::new();
        for value in &values {
            individually.push(quote_value(value, dialect)
                .map_err(|error| TestCaseError::fail(format!("{error:#}")))?);
        }
        prop_assert_eq!(
            &joined,
            &individually.join(" "),
            "shell_join disagreed with shell_quote for dialect {:?}",
            dialect,
        );
    }
}

/// The agreement property is not vacuous: the two elements differ.
///
/// If every element encoded to itself, the joined and individually-quoted
/// forms would agree for any implementation that inserted spaces, and the
/// property would prove nothing. One element that needs quoting is what makes
/// the `join(" ")` in the assertion a real comparison.
#[test]
fn the_agreement_property_has_elements_that_need_quoting() -> Result<()> {
    let values = vec!["a b".to_owned(), "plain".to_owned()];
    let quoted = quote_value("a b", SH)?;
    ensure!(
        quoted != "a b",
        "the witness should require quoting, otherwise this proves nothing"
    );
    assert_eq!(join_values(&values, SH)?, format!("{quoted} plain"));
    Ok(())
}

// ---------------------------------------------------------------------------
// OBL-KIND-GATE
// ---------------------------------------------------------------------------

/// One rejected subject: the value to bind and the kind it should be named as.
///
/// The kind names are the spellings `ValueKind`'s `Display` produces, because
/// the diagnostics interpolate that `Display` and the assertion is on the
/// message a manifest author actually reads.
struct Rejected {
    /// Builds the subject bound to the template's `subject` variable.
    build: fn() -> Value,
    /// The kind name the diagnostic must carry.
    kind: &'static str,
    /// What the subject is, for the failure message.
    description: &'static str,
}

/// Build a `MiniJinja` value that is iterable but not a `Seq`.
///
/// `range(3)` is the template-visible form of this: it is an object reporting
/// `ObjectRepr::Iterable`, so `ValueKind` calls it `iterator` and the gate
/// rejects it, while `try_iter` would hand back `0, 1, 2`.
fn iterable_object() -> Value {
    Value::make_iterable(|| 0_u32..3_u32)
}

/// Subjects `compact` and `shell_join` must reject, with their kind names.
///
/// `map` and `string` are the two D8 exists for: `Value::try_iter()` accepts
/// both — a map yields its keys, a string yields its characters — so an
/// implementation that skipped the kind gate would quietly transform them
/// rather than refusing.
const NON_SEQUENCES: &[Rejected] = &[
    Rejected {
        build: || Value::from_iter(std::iter::once(("a", Value::from(1)))),
        kind: "map",
        description: "a mapping",
    },
    Rejected {
        build: || Value::from("abc"),
        kind: "string",
        description: "a string",
    },
    Rejected {
        build: || Value::from(()),
        kind: "none",
        description: "none",
    },
    Rejected {
        build: || Value::UNDEFINED,
        kind: "undefined",
        description: "an undefined name",
    },
    Rejected {
        build: || Value::from(7),
        kind: "number",
        description: "a number",
    },
    Rejected {
        build: || Value::from(true),
        kind: "bool",
        description: "a boolean",
    },
];

/// Subjects `shell_quote` must reject, with their kind names.
const NON_STRINGS: &[Rejected] = &[
    Rejected {
        build: || Value::from_iter(std::iter::once(("a", Value::from(1)))),
        kind: "map",
        description: "a mapping",
    },
    Rejected {
        build: || Value::from_iter([Value::from(1), Value::from(2)]),
        kind: "sequence",
        description: "a sequence",
    },
    Rejected {
        build: || Value::from(()),
        kind: "none",
        description: "none",
    },
    Rejected {
        build: || Value::UNDEFINED,
        kind: "undefined",
        description: "an undefined name",
    },
    Rejected {
        build: || Value::from(7),
        kind: "number",
        description: "a number",
    },
    Rejected {
        build: || Value::from(true),
        kind: "bool",
        description: "a boolean",
    },
    Rejected {
        build: iterable_object,
        kind: "iterator",
        description: "a MiniJinja object with no string form",
    },
];

/// Render `{{ subject | filter }}` and return the error text.
///
/// A successful render is a failure of the test: the whole point is that the
/// filter refuses, so a value that came back is reported as the surprise it is.
fn rejection(filter: &str, subject: &Value) -> Result<String> {
    let template = format!("{{{{ value | {filter} }}}}");
    match render_with(&template, SH, subject) {
        Ok(rendered) => bail!("{{ value | {filter} }} should have been rejected, got {rendered:?}"),
        Err(error) => Ok(format!("{error:#}")),
    }
}

/// Both sequence filters refuse a non-sequence and name what they got.
///
/// The two filters word the diagnostic differently — `compact expects a
/// sequence` against `shell_join expects a sequence` — but both carry the
/// received kind, which is what the case asserts alongside the expected text.
#[rstest]
#[case::compact("compact", "compact expects a sequence")]
#[case::shell_join("shell_join", "shell_join expects a sequence")]
fn sequence_filters_reject_non_sequences(
    #[case] filter: &str,
    #[case] expectation: &str,
    #[values(
        "a mapping",
        "a string",
        "none",
        "an undefined name",
        "a number",
        "a boolean"
    )]
    description: &str,
) -> Result<()> {
    let rejected = NON_SEQUENCES
        .iter()
        .find(|candidate| candidate.description == description)
        .with_context(|| format!("no rejection fixture for {description}"))?;
    let reported = rejection(filter, &(rejected.build)())?;
    ensure!(
        reported.contains(expectation),
        "{filter} should report {expectation:?} for {description}: {reported}"
    );
    ensure!(
        reported.contains(rejected.kind),
        "{filter} should name the kind {:?} for {description}: {reported}",
        rejected.kind
    );
    Ok(())
}

/// `shell_quote` refuses every non-string subject, including a Jinja object.
///
/// The `args` code is asserted here and not on `compact` because only the
/// recipe-text filters carry it: `compact`'s diagnostic is a collections
/// message with no `netsuke::jinja::shell::args` prefix, and asserting one
/// would be asserting a code that does not exist.
#[rstest]
#[case("a mapping")]
#[case("a sequence")]
#[case("none")]
#[case("an undefined name")]
#[case("a number")]
#[case("a boolean")]
#[case("a MiniJinja object with no string form")]
fn shell_quote_rejects_non_strings(#[case] description: &str) -> Result<()> {
    let rejected = NON_STRINGS
        .iter()
        .find(|candidate| candidate.description == description)
        .with_context(|| format!("no rejection fixture for {description}"))?;
    let reported = rejection("shell_quote", &(rejected.build)())?;
    ensure!(
        reported.contains("shell_quote expects a string"),
        "shell_quote should state its expectation for {description}: {reported}"
    );
    ensure!(
        reported.contains(rejected.kind),
        "shell_quote should name the kind {:?} for {description}: {reported}",
        rejected.kind
    );
    ensure!(
        reported.contains("netsuke::jinja::shell::args"),
        "shell_quote should carry the args code for {description}: {reported}"
    );
    assert_no_stringification(&reported, description);
    Ok(())
}

/// Assert the diagnostic quotes the kind rather than stringifying the value.
///
/// A `to_string` fallback would render a number as `7` inside a *successful*
/// quote, and a boolean as `true`; the diagnostic naming the kind instead is
/// how the caller learns the filter refused rather than coerced. Kept as a
/// helper so the single call site reads as one assertion.
fn assert_no_stringification(reported: &str, description: &str) {
    assert!(
        !reported.contains("expected string"),
        "shell_quote must not fall back to stringifying {description}: {reported}"
    );
}

/// The kind gate is doing work `try_iter` would not do.
///
/// This is the negative control for the two cases above. `try_iter` accepts a
/// map (yielding keys), a string (yielding characters), and an iterable object,
/// so an implementation gated on it alone would accept all three. Showing that
/// the underlying iterator *would* have produced something is what makes the
/// rejection the gate's doing rather than the value being uniterable.
#[test]
fn try_iter_would_have_accepted_three_of_the_rejected_subjects() -> Result<()> {
    for (subject, name) in [
        (
            Value::from_iter(std::iter::once(("a", Value::from(1)))),
            "a mapping",
        ),
        (Value::from("abc"), "a string"),
        (iterable_object(), "an iterable object"),
    ] {
        let items = subject
            .try_iter()
            .with_context(|| format!("{name} should be iterable, or this control proves nothing"))?
            .count();
        assert!(
            items > 0,
            "{name} should yield members, or this control proves nothing"
        );
    }
    // And the gate refuses a mapping anyway, which is the contrast: the
    // rejection is the kind check's doing, not the value being uniterable.
    ensure!(
        rejection(
            "compact",
            &Value::from_iter(std::iter::once(("a", Value::from(1))))
        )?
        .contains("map"),
        "compact must reject a mapping rather than iterate its keys"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// OBL-NO-ESCAPE
// ---------------------------------------------------------------------------

/// Write a one-target manifest binding `value` as the template variable
/// `seam_value`, and return the workspace holding it.
///
/// The value travels in a YAML block scalar rather than being spliced into the
/// template source or quoted as a YAML double-quoted scalar. Both of those
/// alternatives reintroduce the escaping problem this file is about: a value
/// containing `"` or `\` has to be re-escaped for YAML, and a value containing
/// `{{` would be taken as template syntax. A block scalar is literal, so the
/// subject reaches the filter byte for byte.
fn workspace_with_bound_value(value: &str, body: &str) -> Result<tempfile::TempDir> {
    let workspace = tempfile::tempdir().context("create manifest workspace")?;
    let manifest_path = workspace.path().join("Netsukefile");
    let manifest = format!(
        "netsuke_version: \"1.0.0\"\nvars:\n  seam_value: |-\n    {value}\n{body}",
        value = value.replace('\n', "\n    "),
    );
    test_support::fs::write(&manifest_path, manifest).context("write manifest")?;
    Ok(workspace)
}

/// The span the manifest loader rendered for `{{ seam_value | shell_quote }}`.
fn rendered_description(value: &str) -> Result<String> {
    let workspace = workspace_with_bound_value(
        value,
        concat!(
            "targets:\n",
            "  - name: seam\n",
            "    description: \"{{ seam_value | shell_quote(dialect='sh') }}\"\n",
            "    command: echo seam\n",
        ),
    )?;
    let manifest_path = workspace.path().join("Netsukefile");
    let reader: EnvReader = netsuke::manifest::process_env_reader();
    let environment = ManifestEnvironment::new(&reader, Default::default());
    let loaded = manifest::from_path_with_policy_and_environment_and_limits(
        &manifest_path,
        NetworkPolicy::default(),
        &environment,
        Default::default(),
        netsuke::recipe_shell::RecipeShell::Posix,
        None,
    )?;
    Ok(loaded
        .targets
        .first()
        .and_then(|target| target.description.clone())
        .unwrap_or_default())
}

/// The quoter's bytes survive the real manifest loader unchanged.
///
/// The comparison is against `shell_quote`'s own output for the same input, so
/// the assertion fails if either the filter or the loader changes. An escaped
/// `&amp;` inside a recipe would be a silent corruption of exactly the text
/// this feature exists to protect.
#[rstest]
#[case("&")]
#[case("<")]
#[case(">")]
#[case("\"")]
#[case("'")]
#[case("a & b")]
fn quoted_output_survives_manifest_rendering_verbatim(#[case] value: &str) -> Result<()> {
    let expected = quote_value(value, SH)?;
    let observed = rendered_description(value)?;
    ensure!(
        observed == expected,
        "the loader rendered {observed:?} but the quoter produced {expected:?}"
    );
    for escaped in ["&amp;", "&lt;", "&gt;", "&quot;", "&#x27;"] {
        ensure!(
            !observed.contains(escaped),
            "the loader HTML-escaped the quoter's output: {observed}"
        );
    }
    Ok(())
}

/// An auto-escaping environment *does* rewrite the same template.
///
/// This is the negative control that proves the assertion above can detect
/// escaping. The manifest pipeline renders through an unnamed template, so
/// `MiniJinja`'s default callback leaves `<` alone; forcing the callback on shows
/// what the assertion would have caught, and confirms the escape route exists
/// rather than having been removed in some future version.
#[test]
fn an_auto_escaping_environment_rewrites_the_same_template() -> Result<()> {
    let value = "<a & b>";
    let template = "{{ value | shell_quote(dialect='sh') }}";

    let plain = render_with(template, SH, &Value::from(value))?;
    ensure!(
        plain.contains('<'),
        "without auto-escaping the raw character should survive: {plain}"
    );

    let config = StdlibConfig::from_current_dir()?;
    let mut escaping = Environment::new();
    netsuke::stdlib::register_with_config(&mut escaping, config)?;
    escaping.set_auto_escape_callback(|_| AutoEscape::Html);
    let escaped = escaping.render_str(template, context! { value => value })?;
    ensure!(
        escaped != plain,
        "forcing auto-escaping changed nothing, so the control proves nothing"
    );
    ensure!(
        escaped.contains("&lt;") && escaped.contains("&amp;"),
        "the escaping environment should rewrite the significant characters: {escaped}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// OBL-CONTEXT
// ---------------------------------------------------------------------------

/// The generated Ninja `command =` line for a one-target manifest.
///
/// `value` is bound as the template variable `seam_value`; `template` is the
/// command text, which interpolates it.
fn generated_command(value: &str, template: &str) -> Result<String> {
    let workspace = workspace_with_bound_value(
        value,
        &format!(
            concat!(
                "targets:\n",
                "  - name: out\n",
                "    command: \"{template}\"\n",
                "    description: seam\n",
            ),
            template = template.replace('\\', "\\\\").replace('"', "\\\""),
        ),
    )?;
    let manifest_path = workspace.path().join("Netsukefile");
    let run = test_support::netsuke::run_netsuke_in(
        workspace.path(),
        &[
            "--file",
            manifest_path
                .to_str()
                .context("manifest path should be UTF-8")?,
            "generate",
            "--output",
            "out.ninja",
        ],
    )?;
    ensure!(run.success, "generation failed: {}", run.stderr);
    let ninja = std::fs::read_to_string(workspace.path().join("out.ninja"))
        .context("read generated Ninja")?;
    ninja
        .lines()
        .find_map(|line| line.strip_prefix("  command = ").map(str::to_owned))
        .context("generated Ninja should carry a command binding")
}

/// The value the OBL-CONTEXT cases quote: a space and an `=` inside one word.
const CONTEXT_VALUE: &str = "RUSTFLAGS=-D warnings";

/// A filter in unquoted argv position produces text the shell reads correctly.
#[cfg(unix)]
#[rstest]
fn a_filter_in_unquoted_position_yields_one_argument() -> Result<()> {
    let command = generated_command(
        CONTEXT_VALUE,
        "printf '%s\\n' {{ seam_value | shell_quote(dialect='sh') }}",
    )?;

    let shell = posix_shell().context("a POSIX shell is required for this case")?;
    let decoded = run_posix_shell(&shell, &command)?;
    ensure!(
        decoded == format!("{CONTEXT_VALUE}\n"),
        "the shell read {decoded:?} from {command:?}, expected one argument"
    );
    ensure!(
        !command.contains("\"RUSTFLAGS"),
        "the interpolated word must not be enclosed in shell double quotes: {command}"
    );
    Ok(())
}

/// The same filter inside `"..."` is a manifest defect, pinned as such.
///
/// The first draft of the plan's acceptance transcript placed the interpolation
/// inside a pair of double quotes. There the quoter's own quote characters are
/// *data*, and the shell hands the recipe a corrupted argument — the failure
/// mode this obligation exists to name. The case is here so that a future
/// change which silently "fixes" it has to confront the documented answer.
#[cfg(unix)]
#[rstest]
fn a_filter_inside_double_quotes_corrupts_the_argument() -> Result<()> {
    let command = generated_command(
        CONTEXT_VALUE,
        "printf '%s\\n' \"{{ seam_value | shell_quote(dialect='sh') }}\"",
    )?;

    let shell = posix_shell().context("a POSIX shell is required for this case")?;
    let decoded = run_posix_shell(&shell, &command)?;
    ensure!(
        decoded != format!("{CONTEXT_VALUE}\n"),
        "the double-quoted position unexpectedly produced the right argument: {command}"
    );
    ensure!(
        decoded.contains('\'') || decoded.contains('"'),
        "the corruption should be the quoter's literal quote characters: {decoded:?}"
    );
    Ok(())
}

/// The two positions must not produce the same command.
///
/// Both cases above ask the same question of two templates. If the two ever
/// rendered identically, one of the two would be measuring nothing.
#[test]
fn the_two_interpolation_positions_differ() -> Result<()> {
    let unquoted = generated_command(
        CONTEXT_VALUE,
        "printf '%s\\n' {{ seam_value | shell_quote(dialect='sh') }}",
    )?;
    let quoted = generated_command(
        CONTEXT_VALUE,
        "printf '%s\\n' \"{{ seam_value | shell_quote(dialect='sh') }}\"",
    )?;
    assert_ne!(
        unquoted, quoted,
        "the two positions produced the same command, so the pair proves nothing"
    );
    Ok(())
}
