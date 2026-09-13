//! The per-call keyword contract of the four file-reading filters.
//!
//! `max_bytes` and `follow_symlinks` are the only keywords the filters accept,
//! and anything else must be refused rather than ignored: a template that
//! misspells a budget option would otherwise read the file under the
//! operator's default budget while appearing to have narrowed it. minijinja
//! reports the refusal as `TooManyArguments`, the same kind the `which` and
//! `fetch` filters produce for the same mistake. The clause outranks the other
//! arguments of the call as well: `contents` refuses an undeclared keyword
//! before it judges its positional encoding, so a call wrong about both is
//! told about the keyword rather than about the encoding. The clause is shared
//! by all four filters, so it is asserted through the shared table for each
//! entry point rather than for whichever filter a test happened to pick.
use anyhow::{Result, ensure};
use minijinja::ErrorKind;
use rstest::rstest;

use super::fallible;
use super::{CONTENTS, DIGEST, FilterCase, HASH, LINECOUNT, ReadTarget, rejection, render_case};

/// Keywords no file-reading filter declares, each a plausible near-miss of one
/// it does, paired with the name the refusal must quote.
const UNKNOWN_KEYWORDS: [(&str, &str); 3] = [
    ("max_byte=1", "max_byte"),
    ("follow_symlink=true", "follow_symlink"),
    ("limit=1", "limit"),
];

#[rstest]
#[case::contents(CONTENTS)]
#[case::linecount(LINECOUNT)]
#[case::hash(HASH)]
#[case::digest(DIGEST)]
fn an_unknown_keyword_is_rejected(#[case] case: FilterCase) -> Result<()> {
    let (_temp, root) = fallible::filter_workspace()?;
    let file = root.join("file");
    for (kwargs, name) in UNKNOWN_KEYWORDS {
        let err = rejection(
            case,
            render_case(
                case,
                "unknown",
                ReadTarget::new(&root, &file, 1024),
                &case.template_with(kwargs),
            )?,
        )?;
        ensure!(
            err.kind() == ErrorKind::TooManyArguments,
            "{}: '{name}' should be refused as an unexpected keyword but was {:?}",
            case.name,
            err.kind()
        );
        ensure!(
            err.to_string().contains(name),
            "{}: the refusal should name '{name}': {err}",
            case.name
        );
    }
    Ok(())
}

/// A misspelled keyword is refused before `contents` judges its encoding.
///
/// `contents` is the only reading filter whose positional argument can fail on
/// its own, so it is the one place the keyword contract could be pre-empted.
/// `contents('utf-16', max_byte=1)` is wrong about its keyword, and naming that
/// keyword is more use than reporting the encoding the same call also asked
/// for: the misspelling would otherwise surface only on the run after the
/// encoding was corrected. The encoding is judged once the keywords resolve.
#[test]
fn an_unknown_keyword_is_refused_before_an_unsupported_encoding() -> Result<()> {
    let (_temp, root) = fallible::filter_workspace()?;
    let file = root.join("file");
    let err = rejection(
        CONTENTS,
        render_case(
            CONTENTS,
            "keyword_before_encoding",
            ReadTarget::new(&root, &file, 1024),
            "{{ path | contents('utf-16', max_byte=1) }}",
        )?,
    )?;
    ensure!(
        err.kind() == ErrorKind::TooManyArguments,
        "the misspelled keyword should be refused first, but the call failed as {:?}: {err}",
        err.kind()
    );
    ensure!(
        err.to_string().contains("max_byte"),
        "the refusal should name 'max_byte' rather than the encoding: {err}"
    );
    Ok(())
}
