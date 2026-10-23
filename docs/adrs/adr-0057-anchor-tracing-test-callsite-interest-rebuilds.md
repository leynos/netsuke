# Architectural decision record (ADR) 057: Anchor test callsite interest rebuilds

## Status

Accepted.

## Date

2026-10-23

## Context and problem statement

Issue [#927](https://github.com/leynos/netsuke/issues/927) records an
intermittent empty event snapshot from
`runner::manifest_structure_telemetry::tests::manifest_structure_telemetry_carries_only_counts`.
The capture still contained all six structural span fields. The production
function `record_manifest_structure` emits synchronously, so the documented
thread-local boundary of `tracing::subscriber::with_default` does not explain
this loss.

The pinned `tracing-core` 0.1.36 source provides a separate mechanism:
`callsite.rs:544-557` selects `Rebuilder::JustOne` when the live weak registrar
count is at most one; `callsite.rs:564-573` evaluates that path through the
registering thread's default; and `callsite.rs:484-506` rebuilds and publishes
callsite interest. In the demonstrated interleaving, T1 computes `never` under
its DEBUG default, T2 registers a TRACE dispatch and publishes a permissive
rebuild, then T1 publishes its stale `never` result. The callsite stores that
interest at `callsite.rs:358-365`.

The bounded Loom model and a gated fresh-process schedule both demonstrate the
stale write. The real schedule retains the span fields while losing the event.
It does not attribute the separate `stdlib::which` failure rate to this cause.
Upstream issue
[tokio-rs/tracing#3611](https://github.com/tokio-rs/tracing/issues/3611) tracks
the adjacent `JustOne` defect. Its open follow-up
[PR #3614](https://github.com/tokio-rs/tracing/pull/3614) includes a
`register_callsite_contention` regression for the concurrent registration race
and changes the rebuilder to avoid stale publication. The latest released
`tracing-core` remains 0.1.36, so that fix is not yet available to this
workspace.

## Decision drivers

- The telemetry contract requires exactly one event; empty or duplicate
  captures must fail.
- The library must not install a global subscriber, serialize tests, or add
  retries.
- Test helpers already own scoped subscriber lifetimes and are the narrowest
  shared boundary for test-process interest registration.
- A regression must reproduce the unanchored loss and pass with the fix.
- The remedy must retain the normal per-thread `enabled` filtering behaviour.

## Options considered

### Treat the loss as a thread handoff

Changing the capture boundary to follow a spawned emission thread would be
appropriate only if production emitted from that thread. The production call is
synchronous, and the gated worker shows a lost event despite T2's own TRACE
capture and the retained span. This explanation is rejected.

### Weaken or serialize the test

Retries, single-threaded execution, or accepting zero events would hide the
signal and leave the measured lost capture unexplained. This option is rejected.

### Install a global test dispatcher

A global default would cross thread boundaries, but it would change process
state for a library test and would not prevent `JustOne` from publishing stale
interest. This option is rejected.

### Retain two inert test dispatches

The shared capture helpers retain two inert `Dispatch` values in a `OnceLock`.
Their registrars remain alive, but the dispatches are never installed as scoped
or global defaults. This option prevents new registrations from taking
`JustOne` after the anchors are initialized. It is chosen until an upstream
release carries the fix.

## Decision outcome

`test_support::tracing_capture::ensure_test_interest_anchors_registered`
creates two permanent dispatches whose subscriber returns `Interest::never()`,
returns `false` from `enabled`, and advertises `LevelFilter::OFF`. Other
subscriber callbacks are inert. The strong static owners keep both weak
registrars upgradeable when `tracing-core` prunes dead entries, so the live
count remains at least two and `has_just_one` remains false.

Both implementations of `with_test_subscriber` call the anchor function before
constructing capture state. The private span-capture helper in
`runner/dyndep/generation_telemetry.rs` does the same. The root helper's
uninstalled constructor remains crate-private: the capture wrapper and the
fresh-process reproduction worker use it directly, while ordinary callers use
`with_test_subscriber`.

The worker keeps thread orchestration and event snapshots in its private
`threads` module. Those helpers exist only to keep the issue 927 canary and
mitigated schedule aligned; they retain the distinct subscriber installation
boundaries and are not general test-support APIs.

With more than one live registrar, a callsite rebuild walks the registry under
its read lock. Combining an anchor's `never` interest with a different live
interest yields `sometimes`; the current thread's dispatch still applies its own
`enabled` filter. The trade-off is additional registrar walks and a read lock
during `register_callsite`. Subscriber callbacks on that path must not emit
tracing or create a dispatch: a writer-preferring `RwLock` can deadlock when a
callback re-enters registration while a writer waits. The capture layers only
render fields and update mutex-protected buffers.

The anchors cannot cancel a `JustOne` registration that already took its
snapshot. The regression therefore initializes them before T1 installs its
subscriber. The issue 927 worker runs both modes in fresh processes: the
unmitigated canary requires `installed`, six span fields, and a zero-event
snapshot rejected by the exact-cardinality oracle; the mitigated worker requires
`deadline`, the same span fields, and exactly one validated event. Synthetic
zero/one/two-event controls and a seeded duplicate TRACE capture keep the
oracle non-vacuous.

The worker records two separate phases. After constructing its subscriber, T2
sends a boundary signal immediately before `Dispatch::new`. That constructor
allocates its subscriber `Arc` and then calls `tracing-core`'s
`callsite::register_dispatch`; the private registry write-lock attempt has no
public subscriber callback. T2 can be preempted after the boundary signal and
before the constructor reaches that lock, so the signal does not prove lock
contention in every execution. The capture layer separately signals its actual
summary `register_callsite` callback. T1 requires the boundary signal before
releasing its gate. Without anchors, the callback must arrive before release;
with anchors, the callback must arrive only after T1 releases the registry read
lock. A callback drained after both workers join is never accepted as evidence
of overlap. A unit negative control withholds the boundary signal until after
the gate deadline and requires `registration-start-timeout`; this rejects a
late signal but does not rule out pre-emption after an on-time signal. The
bounded Loom model supplies separate ordering coverage for the stale-interest
interleaving.

## Consequences and removal criteria

The thread-local capture limitation remains documented because it is still
real. Nextest's process-per-test layout hides the original cross-test race,
while libtest allows test dispatch registrations to contend in one process.
Neither retry configuration nor test-thread policy changes.

Remove the anchors in a follow-up only after a released `tracing-core` version
contains the upstream fix and the unmitigated fresh-process canary no longer
loses the event. Run that canary before removing the workaround; keep the exact
cardinality and duplicate controls in either case. The current lock resolves
`tracing-core` 0.1.36, and production dependency resolution is unchanged.
