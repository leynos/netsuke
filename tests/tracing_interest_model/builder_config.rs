//! Configure and verify the Loom search bounds used by the tracing model.

use loom::model::Builder;

use super::PREEMPTION_BOUND;

const MAX_MODEL_THREADS: usize = 3;
const MAX_MODEL_BRANCHES: usize = 10_000;
const CHECKPOINT_INTERVAL: usize = usize::MAX;

/// Build a Loom explorer whose completion cannot depend on ambient settings.
///
/// `Builder::new` imports `LOOM_*` environment values. Override every setting
/// that can bound, resume, or instrument enumeration so the reported counts
/// describe the full search for this explicitly bounded model.
pub(super) fn bounded_model_builder() -> Builder {
    let mut builder = Builder::new();
    builder.max_threads = MAX_MODEL_THREADS;
    builder.max_branches = MAX_MODEL_BRANCHES;
    builder.max_permutations = None;
    builder.max_duration = None;
    builder.preemption_bound = Some(PREEMPTION_BOUND);
    builder.checkpoint_file = None;
    builder.checkpoint_interval = CHECKPOINT_INTERVAL;
    builder.expect_explicit_explore = false;
    builder.location = false;
    builder.log = false;
    builder
}

#[test]
fn bounded_builder_disables_ambient_limits_and_checkpointing() {
    let builder = bounded_model_builder();

    assert_eq!(builder.max_threads, MAX_MODEL_THREADS);
    assert_eq!(builder.max_branches, MAX_MODEL_BRANCHES);
    assert_eq!(builder.max_permutations, None);
    assert_eq!(builder.max_duration, None);
    assert_eq!(builder.preemption_bound, Some(PREEMPTION_BOUND));
    assert_eq!(builder.checkpoint_file, None);
    assert_eq!(builder.checkpoint_interval, CHECKPOINT_INTERVAL);
    assert!(!builder.expect_explicit_explore);
    assert!(!builder.location);
    assert!(!builder.log);
}
