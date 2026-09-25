//! The lexical half of the `CONF-1` anti-vacuity rule.
//!
//! `CONF-1` asks that a child RFC's section 5 discharge be group-specific rather
//! than a restatement of the clause it discharges. Two of the three vacuity
//! shapes the rule names are structural, and `clauses` grades them: an empty
//! body, and a body naming none of the helpers its RFC owns. The third — a body
//! that justifies a helper by appealing to Ansible — is a judgement about prose
//! rather than about document structure, and it is made here.
//!
//! Split from `clauses` to keep that module under Whitaker's 400-line
//! `module_max_lines` ceiling, not as a new resolution boundary: `clauses` reads
//! [`deference_phrase`] through `super`, exactly as it read the function when
//! both lived in one file. The seam is the kind of judgement rather than the
//! size of the code, which is why the unit tests below travel with it: they pin
//! the predicate against sentences, and every other test in this module tree is
//! exercised through a document.

/// The claim that a helper exists because Ansible has one.
///
/// This is literally the acceptance criterion the task exists to enforce: RFC
/// 0006 is a survey of Ansible's standard library, and a child RFC that
/// justifies a helper by appealing to Ansible rather than to Netsuke's own
/// contract has not made a decision, only deferred one. It is a substring
/// search, so leaving it to review would be indefensible.
///
/// Each phrase is matched at a leading word boundary and case-insensitively.
/// The boundary keeps `Unlike Ansible` — which contains `like Ansible` and says
/// the opposite of what this check looks for — from being flagged as the appeal
/// it argues against. Case is folded because the appeal is as likely to open a
/// sentence as to sit inside one: `As Ansible does` and `as Ansible does` are
/// one phrase, and a case-sensitive search would catch only the second.
///
/// A leading boundary alone does not clear `such as Ansible`, because `as` is
/// its own word there — the phrase is the tail of the compound preposition —
/// and the sentence introduces an example rather than a justification. That
/// shape is excluded by name in [`deference_phrase`]. A trailing boundary is
/// deliberately *not* required: `as Ansible does` is the shape the check exists
/// to catch, and it is this plan's own recorded seeded fault, so tightening the
/// right-hand side would delete a control that has been proven to fire.
///
/// Each entry is a `(pattern, display)` pair. The pattern is the lowercase form
/// the folded body is searched for; the display form is what the diagnostic
/// quotes, because `as ansible` in an error message reads as a typo rather than
/// as the phrase the check names.
const DEFERENCE_PHRASES: [(&str, &str); 2] = [
    ("as ansible", "as Ansible"),
    ("like ansible", "like Ansible"),
];

/// The exemplar idiom that contains `as Ansible` without deferring to it.
///
/// `such as Ansible` names a source of examples, not a reason to have a helper.
/// Matched immediately before the phrase, on the lowercased text, so the
/// exclusion cannot swallow a genuine appeal that happens to follow one of
/// these words elsewhere in the sentence, and so `Such as Ansible` is cleared
/// as its lowercase form is.
///
/// Only `such ` is listed. An earlier draft also carried `with ` and `in `, for
/// the shapes `as with Ansible` and `as in Ansible`; both were unreachable. The
/// scan finds `as Ansible` or `like Ansible` and then reads the text *before*
/// the match, so the prefix it tests is the text preceding `as` — for those two
/// shapes that is the empty string, never `with ` or `in `. The entries could
/// not change any answer, and their presence made the doc comment look as
/// though the two shapes were handled when only the boundary rule was doing any
/// work. `as with Ansible` and `as in Ansible` are cleared because neither
/// contains the substring `as Ansible` or `like Ansible` at all.
const EXEMPLAR_PREFIXES: [&str; 1] = ["such "];

/// Whether `body` justifies a helper by appeal to Ansible.
///
/// Returns the phrase's display form, so the diagnostic can quote it. That form
/// is canonical rather than the text as it appeared: the match is made against
/// a folded copy of the body, so a sentence-initial `As Ansible does` would
/// otherwise be quoted back in whatever case the child RFC happened to use, and
/// a diagnostic naming three different spellings of one phrase as it fires
/// three times is harder to read than one naming a single shape.
pub(super) fn deference_phrase(body: &str) -> Option<&'static str> {
    // Lowercased once per call rather than per candidate phrase. The offsets
    // this yields are offsets into the folded text, which is what the
    // surrounding-token tests below read; `to_lowercase` is not guaranteed to
    // preserve byte length for every script, but the phrases and the context
    // that decides their boundaries are ASCII throughout this corpus.
    let folded = body.to_lowercase();
    DEFERENCE_PHRASES
        .into_iter()
        .find(|(pattern, _)| {
            folded.match_indices(*pattern).any(|(at, _)| {
                let Some(prefix) = folded.get(..at) else {
                    return false;
                };
                // A boundary is the start of the text, or a character that cannot
                // continue a word. `is_alphanumeric` alone would treat the `_` in
                // `unlike_ansible` as a break, and the underscore is a word
                // character in every identifier this corpus uses.
                let bounded = !prefix
                    .chars()
                    .next_back()
                    .is_some_and(|ch| ch.is_alphanumeric() || ch == '_');
                let exemplar = EXEMPLAR_PREFIXES
                    .iter()
                    .any(|prefix_word| prefix.ends_with(prefix_word));
                bounded && !exemplar
            })
        })
        .map(|(_, display)| display)
}

#[cfg(test)]
mod deference_tests {
    //! Unit tests for [`deference_phrase`](super::deference_phrase).
    //!
    //! The predicate is the mechanical half of this plan's own acceptance
    //! criterion, and it is a lexical search over prose: the two false shapes
    //! below are sentences a child RFC would plausibly write, and both read as
    //! the reverse of the appeal it exists to catch.

    use super::deference_phrase;

    /// An appeal to Ansible is caught, and quoted back in the diagnostic.
    ///
    /// `as Ansible does` is the recorded seeded fault for `CONF-1`, so it is
    /// pinned here: a change that stopped catching it would leave that control
    /// green and empty.
    #[test]
    fn an_appeal_is_flagged() {
        assert_eq!(deference_phrase("as Ansible does"), Some("as Ansible"));
        assert_eq!(deference_phrase("as Ansible"), Some("as Ansible"));
        assert_eq!(
            deference_phrase("like Ansible's fileglob"),
            Some("like Ansible")
        );
        assert_eq!(
            deference_phrase("This is like Ansible"),
            Some("like Ansible")
        );
    }

    /// The same appeal opening a sentence is caught too.
    ///
    /// The phrases are matched case-insensitively, so capitalizing the first
    /// word does not hide the appeal. Each case asserts the canonical lowercase
    /// phrase is what comes back, because that is what the diagnostic quotes.
    #[test]
    fn a_capitalized_appeal_is_flagged() {
        assert_eq!(deference_phrase("As Ansible does"), Some("as Ansible"));
        assert_eq!(deference_phrase("Like Ansible"), Some("like Ansible"));
        assert_eq!(
            deference_phrase("AS ANSIBLE DOES"),
            Some("as Ansible"),
            "a shouted appeal is the same appeal"
        );
    }

    /// A sentence that argues *against* the appeal is not an appeal.
    ///
    /// `Unlike Ansible` contains `like Ansible`, and flagging it would fail a
    /// subsection that has made exactly the decision this check demands.
    #[test]
    fn the_reverse_of_an_appeal_is_not_flagged() {
        assert_eq!(deference_phrase("Unlike Ansible, Netsuke has none"), None);
        assert_eq!(deference_phrase("unlike_ansible"), None);
        assert_eq!(deference_phrase("unlikeAnsible"), None);
    }

    /// Naming Ansible as a source of examples is not deference to it.
    ///
    /// `such as Ansible` introduces an instance, and its `as` is its own word,
    /// so the leading boundary alone does not clear it. The `such ` entry in
    /// [`EXEMPLAR_PREFIXES`](super::EXEMPLAR_PREFIXES) is what does, and the
    /// last assertion below is that entry's liveness check: it carries the same
    /// `as Ansible does` tail under a prefix that is not an exemplar, so it is
    /// caught. An entry that stopped matching would turn the first two
    /// assertions red instead of leaving them green and empty.
    ///
    /// `as with Ansible` and `as in Ansible` are cleared by the phrase search
    /// itself — neither contains `as Ansible` or `like Ansible` as a substring
    /// — so they are not this predicate's cases to argue about and are not
    /// asserted here as though the exemplar list were clearing them.
    #[test]
    fn an_example_list_is_not_flagged() {
        assert_eq!(deference_phrase("such as Ansible does today"), None);
        assert_eq!(deference_phrase("helpers such as Ansible has"), None);
        assert_eq!(
            deference_phrase("Such as Ansible does"),
            None,
            "the exemplar prefix is matched against the folded text"
        );
        assert_eq!(
            deference_phrase("today, as Ansible does"),
            Some("as Ansible"),
            "the same words under a nonexemplar prefix are the appeal"
        );
    }

    /// A sentence mentioning Ansible for another reason is not flagged.
    #[test]
    fn an_unrelated_mention_is_not_flagged() {
        assert_eq!(deference_phrase("because Ansible has one"), None);
        assert_eq!(deference_phrase("Ansible"), None);
        assert_eq!(deference_phrase(""), None);
    }
}
