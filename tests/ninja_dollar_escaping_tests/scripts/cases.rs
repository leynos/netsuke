//! Table data for the script recipe integration cases.
//!
//! Each enum names one hostile or boundary input so a case label states what
//! it covers, and the case functions take typed values rather than loose
//! strings. The text itself lives beside the variant it describes.

/// Name the marker a hostile path is lowered through.
#[cfg(unix)]
#[derive(Clone, Copy)]
pub(super) enum Marker {
    /// The `$out` marker.
    Outs,
    /// The `$in` marker.
    Ins,
}

#[cfg(unix)]
impl Marker {
    /// Return the marker as it is written in a script.
    pub(super) const fn token(self) -> &'static str {
        match self {
            Self::Outs => "{{ outs }}",
            Self::Ins => "{{ ins }}",
        }
    }
}

/// Name which marker carries the hostile path.
#[cfg(unix)]
#[derive(Clone, Copy)]
pub(super) enum HostileSite {
    /// The hostile text is the output path, which is the target name.
    OutputPath,
    /// The hostile text is a declared input path.
    InputPath,
}

#[cfg(unix)]
impl HostileSite {
    /// The path text whose separator must stay inside one argument.
    const HOSTILE_PATH: &'static str = "a; touch injected; echo b";

    /// Return the target name the manifest declares.
    pub(super) const fn target(self) -> &'static str {
        match self {
            Self::OutputPath => Self::HOSTILE_PATH,
            Self::InputPath => "out",
        }
    }

    /// Return the declared source path, when the case has one.
    pub(super) const fn source(self) -> Option<&'static str> {
        match self {
            Self::OutputPath => None,
            Self::InputPath => Some(Self::HOSTILE_PATH),
        }
    }
}

/// Name an unmatched quote that precedes a script marker.
#[cfg(unix)]
#[derive(Clone, Copy)]
pub(super) enum Comment {
    /// An unmatched apostrophe.
    Apostrophe,
    /// An unmatched double quote.
    DoubleQuote,
}

#[cfg(unix)]
impl Comment {
    /// Return the comment line that leaves its quote open.
    pub(super) const fn line(self) -> &'static str {
        match self {
            Self::Apostrophe => "# unmatched apostrophe '",
            Self::DoubleQuote => "# unmatched double quote \"",
        }
    }
}

/// Name a control character that must be rejected in a command.
#[derive(Clone, Copy)]
pub(super) enum ControlCharacter {
    /// A line feed, which would start a new Ninja statement.
    Newline,
    /// A carriage return, which Ninja cannot represent.
    CarriageReturn,
}

impl ControlCharacter {
    /// Return a command that carries this control character.
    pub(super) const fn command(self) -> &'static str {
        match self {
            Self::Newline => "echo safe\nbuild injected: action",
            Self::CarriageReturn => "echo safe\rbuild injected: action",
        }
    }
}

/// Name a path character that Ninja cannot represent.
#[derive(Clone, Copy)]
pub(super) enum UnsafePath {
    /// A dollar sign, which Ninja reads as a variable reference.
    Dollar,
    /// A colon, which separates an edge from its outputs.
    Colon,
    /// A pipe, which starts an implicit dependency list.
    Pipe,
    /// A tab, which separates rule fields.
    Tab,
    /// A NUL byte, which no path may contain.
    Nul,
    /// A carriage return.
    CarriageReturn,
    /// A line feed.
    Newline,
}

impl UnsafePath {
    /// Return a path containing this character.
    pub(super) const fn text(self) -> &'static str {
        match self {
            Self::Dollar => "input$file",
            Self::Colon => "input:file",
            Self::Pipe => "input|file",
            Self::Tab => "input\tfile",
            Self::Nul => "input\0file",
            Self::CarriageReturn => "input\rfile",
            Self::Newline => "input\nfile",
        }
    }
}

/// Name one script whose escapes must keep a following marker safe.
#[cfg(unix)]
#[derive(Clone, Copy)]
pub(super) enum EscapedScript {
    /// A backtick escaped inside a `$()` frame in the body.
    EscapedBacktickInNestedFrame,
    /// A `\\` pair that leaves a backslash before a parenthesis.
    SurvivingBackslashBeforeParenthesis,
    /// A `\\` pair inside single quotes leaves a literal backslash only.
    SurvivingBackslashInSingleQuotes,
    /// A backslash-dollar exposes a command substitution to the body's parse.
    ExposedSubstitutionAfterEscapedDollar,
    /// A comment inside the body hides the parenthesis on its line.
    CommentInsideBodyHidesParenthesis,
}

#[cfg(unix)]
impl EscapedScript {
    /// Return the script line the manifest declares.
    pub(super) const fn script(self) -> &'static str {
        match self {
            Self::EscapedBacktickInNestedFrame => {
                r#"printf '<%s>\n' `printf %s "$(printf %s '\`')"` {{ ins }}"#
            }
            Self::SurvivingBackslashBeforeParenthesis => {
                r"printf '<%s>\n' $(printf %s `printf %s \\)`) {{ ins }}"
            }
            Self::SurvivingBackslashInSingleQuotes => {
                r"printf '<%s>\n' $(printf %s `printf %s '\\' ')'`) {{ ins }}"
            }
            Self::ExposedSubstitutionAfterEscapedDollar => {
                r#"printf '<%s>\n' $(printf %s `printf %s "\$(printf %s ")")"`) {{ ins }}"#
            }
            Self::CommentInsideBodyHidesParenthesis => {
                // The manifest renders this inside a block scalar, so the
                // continuation line carries the same six-space indent the
                // case function adds to the first line.
                "printf '<%s>\\n' $(printf %s `printf x # )\n      `) {{ ins }}"
            }
        }
    }

    /// Return the exact standard output the generated command must print.
    pub(super) const fn expected(self) -> &'static [u8] {
        match self {
            Self::EscapedBacktickInNestedFrame => b"<`>\n<a; touch injected; echo b>\n",
            Self::SurvivingBackslashBeforeParenthesis
            | Self::ExposedSubstitutionAfterEscapedDollar => b"<)>\n<a; touch injected; echo b>\n",
            Self::SurvivingBackslashInSingleQuotes => b"<\\)>\n<a; touch injected; echo b>\n",
            Self::CommentInsideBodyHidesParenthesis => b"<x>\n<a; touch injected; echo b>\n",
        }
    }
}
