//! Bounded model of tracing-core callsite-interest registration under concurrency.
//!
//! This models the pinned `tracing-core` 0.1.36 registrar state and callsite
//! cache. It deliberately omits allocator timing, the real thread-local
//! dispatcher runtime, and complete `tracing-subscriber` stacks; the companion
//! fresh-process test exercises those production callsites. The source map is
//! `dispatcher.rs:472-480` for dispatch registration,
//! `callsite.rs:236-252` for callsite publication,
//! `callsite.rs:484-506` for interest rebuild and publication,
//! `callsite.rs:544-573` for rebuilder selection and registrar traversal,
//! `callsite.rs:358-365` for the `SeqCst` callsite-interest write, and
//! `subscriber.rs:652-664` for `Interest::and`. Loom 0.7.2 has no `Weak`, so
//! weak registrars hold a Loom-tracked liveness token that does not retain the
//! dispatch; dropping the dispatch marks that token dead for pruning.

use loom::{
    model::Builder,
    sync::{
        Arc, Condvar, Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard,
        atomic::{AtomicBool, AtomicU8, Ordering},
    },
    thread,
};
use std::{
    io::Write,
    sync::{
        Arc as StdArc,
        atomic::{AtomicUsize, Ordering as StdOrdering},
    },
};

const PREEMPTION_BOUND: usize = 3;
const CACHED_NEVER: u8 = 0;
const CACHED_SOMETIMES: u8 = 1;
const CACHED_ALWAYS: u8 = 2;
const CACHED_UNREGISTERED: u8 = 3;

/// Interest returned by one dispatch for the modeled summary callsite.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Interest {
    Never,
    Sometimes,
    Always,
}

impl Interest {
    /// Combine two dispatch interests as `tracing_core::subscriber::Interest::and` does.
    fn and(self, other: Self) -> Self {
        if self == other { self } else { Self::Sometimes }
    }

    /// Encode an interest for the modeled relaxed callsite cache.
    const fn encode(self) -> u8 {
        match self {
            Self::Never => CACHED_NEVER,
            Self::Sometimes => CACHED_SOMETIMES,
            Self::Always => CACHED_ALWAYS,
        }
    }

    /// Decode a cached interest written by a callsite rebuild.
    const fn decode(encoded: u8) -> Option<Self> {
        match encoded {
            CACHED_NEVER => Some(Self::Never),
            CACHED_SOMETIMES => Some(Self::Sometimes),
            CACHED_ALWAYS => Some(Self::Always),
            _ => None,
        }
    }
}

/// One live dispatch registrar and its interest for the summary event.
#[derive(Debug)]
struct Dispatch {
    id: usize,
    interest: Interest,
    is_live: Arc<AtomicBool>,
}

impl Dispatch {
    /// Create a live thread-default dispatch with one callsite interest.
    fn new(id: usize, interest: Interest) -> Arc<Self> {
        Arc::new(Self {
            id,
            interest,
            is_live: Arc::new(AtomicBool::new(true)),
        })
    }
}

impl Drop for Dispatch {
    /// Mark the logical weak registrar dead when its dispatch owner drops.
    fn drop(&mut self) {
        self.is_live.store(false, Ordering::Release);
    }
}

/// Weak registrar that preserves identity without retaining its dispatch.
#[derive(Debug)]
struct WeakRegistrar {
    id: usize,
    interest: Interest,
    is_live: Arc<AtomicBool>,
}

impl WeakRegistrar {
    /// Create a logical weak registrar from a live dispatch owner.
    fn new(dispatch: &Arc<Dispatch>) -> Self {
        Self {
            id: dispatch.id,
            interest: dispatch.interest,
            is_live: Arc::clone(&dispatch.is_live),
        }
    }

    /// Upgrade the registrar only while its dispatch owner remains live.
    fn upgrade(&self) -> Option<RegistrarView> {
        self.is_live
            .load(Ordering::Acquire)
            .then_some(RegistrarView {
                id: self.id,
                interest: self.interest,
            })
    }
}

/// Interest and identity yielded by a successful logical weak upgrade.
#[derive(Clone, Copy)]
struct RegistrarView {
    id: usize,
    interest: Interest,
}

/// Return value selected by the pinned registry's dispatcher-count fast path.
enum Rebuilder<'a> {
    JustOne,
    Read(RwLockReadGuard<'a, Vec<WeakRegistrar>>),
    Write(RwLockWriteGuard<'a, Vec<WeakRegistrar>>),
}

/// State shared by dispatch registration and the first summary callsite.
struct Registry {
    dispatchers: RwLock<Vec<WeakRegistrar>>,
    has_just_one: AtomicBool,
    callsite_published: AtomicBool,
    cached_interest: AtomicU8,
}

impl Registry {
    /// Create an empty registry with an unregistered summary callsite.
    fn new() -> Self {
        Self {
            dispatchers: RwLock::new(Vec::new()),
            has_just_one: AtomicBool::new(true),
            callsite_published: AtomicBool::new(false),
            cached_interest: AtomicU8::new(CACHED_UNREGISTERED),
        }
    }

    /// Select `JustOne` or hold the registry read lock for multiple dispatches.
    fn rebuilder(&self) -> Rebuilder<'_> {
        if self.has_just_one.load(Ordering::SeqCst) {
            Rebuilder::JustOne
        } else {
            Rebuilder::Read(
                self.dispatchers
                    .read()
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
            )
        }
    }

    /// Publish a dispatcher and rebuild a callsite while holding the write lock.
    fn register_dispatch(&self, dispatch: &Arc<Dispatch>) {
        let mut dispatchers = self
            .dispatchers
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        dispatchers.retain(|registrar| registrar.upgrade().is_some());
        dispatchers.push(WeakRegistrar::new(dispatch));
        self.has_just_one
            .store(dispatchers.len() <= 1, Ordering::SeqCst);
        let rebuilder = Rebuilder::Write(dispatchers);
        if self.callsite_published.load(Ordering::Acquire) {
            let interest = Self::rebuilt_interest(&rebuilder, dispatch);
            self.set_interest(interest);
        }
    }

    /// Publish the callsite, compute T1's interest, pause, then perform its write.
    fn register_summary_callsite(&self, thread_default: &Arc<Dispatch>, gate: &Gate) {
        self.callsite_published.store(true, Ordering::Release);
        let rebuilder = self.rebuilder();
        let interest = Self::rebuilt_interest(&rebuilder, thread_default);
        gate.pause_t1_until_t2_attempts_registration();
        self.set_interest(interest);
        drop(rebuilder);
    }

    /// Recompute interest using a `JustOne`, read-locked, or write-locked view.
    fn rebuilt_interest(rebuilder: &Rebuilder<'_>, thread_default: &Arc<Dispatch>) -> Interest {
        let mut interest: Option<Interest> = None;
        Self::for_each(rebuilder, thread_default, |dispatch| {
            interest = Some(interest.map_or(dispatch, |current| current.and(dispatch)));
        });
        interest.unwrap_or(Interest::Never)
    }

    /// Visit a thread-local default or every upgraded weak registrar.
    fn for_each(
        rebuilder: &Rebuilder<'_>,
        thread_default: &Arc<Dispatch>,
        mut visit: impl FnMut(Interest),
    ) {
        match rebuilder {
            Rebuilder::JustOne => visit(thread_default.interest),
            Rebuilder::Read(dispatchers) => visit_weak_registrars(dispatchers, &mut visit),
            Rebuilder::Write(dispatchers) => visit_weak_registrars(dispatchers, &mut visit),
        }
    }

    /// Store a callsite interest with the pinned `DefaultCallsite` ordering.
    fn set_interest(&self, interest: Interest) {
        self.cached_interest
            .store(interest.encode(), Ordering::SeqCst);
    }

    /// Read the current callsite interest using the pinned atomic ordering.
    fn cached_interest(&self) -> Option<Interest> {
        Interest::decode(self.cached_interest.load(Ordering::SeqCst))
    }

    /// Confirm a dispatch remains reachable through a weak registrar.
    fn contains_dispatch(&self, expected: &Arc<Dispatch>) -> bool {
        self.dispatchers
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .any(|registrar| {
                registrar
                    .upgrade()
                    .is_some_and(|dispatch| dispatch.id == expected.id)
            })
    }
}

/// Visit each registrar that still has a strong dispatch owner.
fn visit_weak_registrars(registrars: &[WeakRegistrar], visit: &mut impl FnMut(Interest)) {
    registrars
        .iter()
        .filter_map(WeakRegistrar::upgrade)
        .for_each(|dispatch| visit(dispatch.interest));
}

/// Coordinate the T1 snapshot and T2 dispatcher-registration schedule.
#[derive(Default)]
struct Gate {
    state: Mutex<GateState>,
    changed: Condvar,
}

/// Record the publication and rebuild phases that control the model schedule.
#[derive(Default)]
struct GateState {
    t1_paused: bool,
    t2_attempted: bool,
}

impl Gate {
    /// Pause T1 until T2 attempts to register its dispatcher.
    fn pause_t1_until_t2_attempts_registration(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.t1_paused = true;
        self.changed.notify_all();
        while !state.t2_attempted {
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }

    /// Wait for T1 to publish its callsite before T2 requests the write lock.
    fn t2_attempt_registration_after_t1_pauses(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        while !state.t1_paused {
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        state.t2_attempted = true;
        self.changed.notify_all();
    }
}

/// Summarize schedules explored and executions that lose T2's TRACE event.
#[derive(Debug)]
struct ModelCounts {
    executions: usize,
    lost_events: usize,
}

/// Explore one configuration under the same explicit preemption bound.
fn explore(anchors_are_live: bool) -> ModelCounts {
    let executions = StdArc::new(AtomicUsize::new(0));
    let lost_events = StdArc::new(AtomicUsize::new(0));
    let counted_executions = StdArc::clone(&executions);
    let counted_losses = StdArc::clone(&lost_events);
    let mut builder = Builder::new();
    builder.max_threads = 3;
    builder.max_branches = 10_000;
    builder.preemption_bound = Some(PREEMPTION_BOUND);
    builder.check(move || {
        counted_executions.fetch_add(1, StdOrdering::Relaxed);
        if one_execution_loses_event(anchors_are_live) {
            counted_losses.fetch_add(1, StdOrdering::Relaxed);
        }
    });
    ModelCounts {
        executions: executions.load(StdOrdering::Relaxed),
        lost_events: lost_events.load(StdOrdering::Relaxed),
    }
}

/// Run T1's stale registration against T2's live TRACE dispatcher.
fn one_execution_loses_event(anchors_are_live: bool) -> bool {
    let registry = Arc::new(Registry::new());
    let anchors = if anchors_are_live {
        let first = Dispatch::new(1, Interest::Never);
        let second = Dispatch::new(2, Interest::Never);
        registry.register_dispatch(&first);
        registry.register_dispatch(&second);
        vec![first, second]
    } else {
        Vec::new()
    };
    let debug_default = Dispatch::new(3, Interest::Never);
    registry.register_dispatch(&debug_default);
    let trace_default = Dispatch::new(4, Interest::Always);
    let gate = Arc::new(Gate::default());

    let t1_registry = Arc::clone(&registry);
    let t1_default = Arc::clone(&debug_default);
    let t1_gate = Arc::clone(&gate);
    let t1 = thread::spawn(move || {
        t1_registry.register_summary_callsite(&t1_default, &t1_gate);
    });

    let t2_registry = Arc::clone(&registry);
    let t2_default = Arc::clone(&trace_default);
    let t2_gate = Arc::clone(&gate);
    let t2 = thread::spawn(move || {
        t2_gate.t2_attempt_registration_after_t1_pauses();
        t2_registry.register_dispatch(&t2_default);
    });

    assert!(t1.join().is_ok(), "T1 modeled thread must not panic");
    assert!(t2.join().is_ok(), "T2 modeled thread must not panic");
    let cached = registry.cached_interest();
    let trace_is_live = registry.contains_dispatch(&trace_default);
    let t2_thread_default_is_enabled = trace_default.interest != Interest::Never;
    let _live_anchor_owners = anchors;
    trace_is_live && cached == Some(Interest::Never) && t2_thread_default_is_enabled
}

#[test]
fn weak_registrars_prevent_stale_just_one_interest_overwrites() {
    let unanchored = explore(false);
    let anchored = explore(true);
    let mut output = std::io::stdout().lock();
    writeln!(
        output,
        "ISSUE927_LOOM preemption_bound={PREEMPTION_BOUND} unanchored_executions={} unanchored_lost={} anchored_executions={} anchored_lost={}",
        unanchored.executions,
        unanchored.lost_events,
        anchored.executions,
        anchored.lost_events
    )
    .expect("the Loom execution summary should be writable");
    assert!(
        unanchored.executions > 0 && unanchored.lost_events > 0,
        "the unanchored model must explore a live TRACE registrar with cached Never: {unanchored:?}"
    );
    assert!(
        anchored.executions > 0 && anchored.lost_events == 0,
        "two live anchors must prevent a lost TRACE event: {anchored:?}"
    );
}
