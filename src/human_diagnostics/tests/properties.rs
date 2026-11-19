//! Property tests for the ordinary cause-chain fallback.
//!
//! `distinct_causes` drops a message equal to the outer context or to the link
//! kept before it, and `CauseChain::from_messages` turns a message list into
//! the `source()` chain `miette` walks. Both rules apply to chains of any
//! shape, so these properties generate arbitrary chains rather than repeating
//! the named examples beside them.

use proptest::prelude::*;

use crate::human_diagnostics::cause_chain::{CauseChain, distinct_causes};

/// Generate short chains from a small alphabet so repeats occur often.
///
/// The narrow alphabet is deliberate: duplicates and a cause equal to the
/// outer message are the cases the deduplication rule exists for, and a wide
/// alphabet would generate them too rarely to exercise the rule.
fn chain_messages() -> impl Strategy<Value = Vec<String>> {
    let word = prop::sample::select(vec!["alpha", "beta", "gamma"]).prop_map(String::from);
    prop::collection::vec(word, 1..6)
}

/// Build a failure whose chain is `messages`, outermost first.
fn chained_error(messages: &[String]) -> anyhow::Error {
    // `anyhow::Error::msg` creates the root; each later `.context` becomes a
    // new outermost layer, so the list is folded from its tail.
    let mut inner_to_outer = messages.iter().rev();
    let root = inner_to_outer.next().cloned().unwrap_or_default();
    inner_to_outer.fold(anyhow::Error::msg(root), |error, message| {
        error.context(message.clone())
    })
}

proptest! {
    /// Deduplication keeps every invariant for any chain shape.
    #[test]
    fn distinct_causes_keeps_its_invariants(messages in chain_messages()) {
        let error = chained_error(&messages);
        let outer = error.to_string();
        let causes = distinct_causes(&error);

        prop_assert!(
            causes.iter().all(|cause| cause != &outer),
            "a cause equal to the outer message survived: {causes:?}"
        );
        prop_assert!(
            causes
                .windows(2)
                .all(|pair| matches!(pair, [left, right] if left != right)),
            "two adjacent causes are equal: {causes:?}"
        );
        prop_assert!(
            causes.iter().all(|cause| !cause.is_empty()),
            "an empty cause survived: {causes:?}"
        );
        prop_assert!(
            causes
                .iter()
                .all(|cause| messages.iter().any(|message| message == cause)),
            "a cause was invented rather than kept: {causes:?}"
        );

        // Walking the input once for each kept cause proves the output is an
        // order-preserving subsequence, not a reordering of the same values.
        let mut remaining = messages.iter();
        for cause in &causes {
            prop_assert!(
                remaining.any(|message| message == cause),
                "the causes are not an ordered subsequence: {causes:?} of {messages:?}"
            );
        }
    }

    /// A chain links exactly the messages it was built from.
    #[test]
    fn cause_chain_walks_every_message_in_order(messages in chain_messages()) {
        let head = CauseChain::from_messages(messages.clone())
            .expect("a non-empty message list must build a chain");

        let mut walked = Vec::new();
        let mut current: Option<&(dyn std::error::Error + 'static)> = Some(&head);
        while let Some(node) = current {
            walked.push(node.to_string());
            current = node.source();
        }

        prop_assert_eq!(walked, messages);
    }
}
