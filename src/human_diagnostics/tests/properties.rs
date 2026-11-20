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

/// Compute the cause sequence the documented rule selects.
///
/// The report prints the outermost message above the chain, so the chain
/// beneath it starts at index 1. A message equal to that printed line, or to
/// the cause kept immediately before it, is dropped.
///
/// Stating the rule here, independently of `distinct_causes`, lets the property
/// assert the whole sequence rather than only that no bad cause survives. The
/// weaker form passed vacuously for a deduplication that returned nothing: for
/// `["alpha", "beta"]` the documented answer is `["beta"]`, but every
/// "nothing unwanted survived" assertion also holds for an empty result.
fn documented_causes(messages: &[String]) -> Vec<String> {
    let mut causes = Vec::new();
    let Some((outer, tail)) = messages.split_first() else {
        return causes;
    };
    let mut previous = outer;
    for message in tail {
        if message != outer && message != previous {
            previous = message;
            causes.push(message.clone());
        }
    }
    causes
}

proptest! {
    /// Deduplication keeps exactly the documented causes for any chain shape.
    #[test]
    fn distinct_causes_returns_the_documented_sequence(messages in chain_messages()) {
        let error = chained_error(&messages);
        let causes = distinct_causes(&error);

        prop_assert_eq!(
            causes,
            documented_causes(&messages),
            "the reported causes must match the documented rule for {:?}",
            messages
        );
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
