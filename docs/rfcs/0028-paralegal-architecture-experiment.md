# RFC 0028: Evaluate Paralegal for architectural flow checks

## Preamble

- **RFC number:** 0028
- **Status:** Proposed
- **Created:** 2026-09-19
- **Scope:** Bounded experiment, not analyser adoption.
- **Decision record:**
  [ADR-031](../adr-031-gate-paralegal-on-measured-evidence.md).
- **Delivery:** [Architecture roadmap](../roadmap-hexagonal-hardening.md), phase
  29.

## Summary

Test whether Paralegal provides useful, affordable evidence about semantic flows
that the structural checker and Rust types do not already enforce. The first
question is whether it can analyse actual Netsuke production code with Netsuke's
required Rust toolchain. A failed or inconclusive compatibility check stops the
pilot. It does not authorize a compiler downgrade or block the hardening in [RFC
0026](0026-hexagonal-domain-hardening.md) and [RFC
0027](0027-executable-architecture-contract.md).

No Paralegal dependency, marker, installation, workflow, or mandatory check is
introduced by this RFC. All thresholds below are proposed experimental decision
criteria, not measured results or claims that the analyser will meet them.

## Baseline and uncertainty

On 2026-09-19, source inspection found:

| Component | Inspected revision                         | Declared compiler    |
| --------- | ------------------------------------------ | -------------------- |
| Netsuke   | `79545e124b4a13dbe8352fd23df97619a91a0b5c` | `nightly-2026-08-23` |
| Paralegal | `c96efb341aba35c16b02e5746681d9c8fb74fe9c` | `nightly-2026-04-20` |

_Table 1: Source-inspected pins, not an executed compatibility matrix._

[Netsuke's toolchain][netsuke-pin], [ADR-006][adr-006], and `AGENTS.md` require
the pinned compiler's Polonius and next-generation trait-solver behaviour.
[Paralegal's pin][paralegal-pin] also requests `rustc-dev` and `rust-src`. Its
[installation documentation][paralegal-readme] describes building against that
pin. Different dates identify a risk; they do not independently prove
compatibility or incompatibility. This RFC records the result as **not run**.

[netsuke-pin]: ../../rust-toolchain.toml
[adr-006]: ../adr-006-adopt-polonius-nightly-toolchain.md
[paralegal-pin]: https://github.com/brownsys/paralegal/blob/c96efb341aba35c16b02e5746681d9c8fb74fe9c/rust-toolchain.toml
[paralegal-readme]: https://github.com/brownsys/paralegal/blob/c96efb341aba35c16b02e5746681d9c8fb74fe9c/README.md

## Hypotheses and decision gates

### P0: Required-toolchain compatibility comes first

Hypothesis: a pinned, maintainable Paralegal analyser can compile and extract a
non-empty dependence graph from actual Netsuke production code while using the
compiler required by Netsuke.

The preflight must perform these steps before designing production policies:

1. Freeze the Netsuke revision, lockfile, required compiler, target, features,
   and relevant build configuration. Record the actual `rustc -vV` output,
   sysroot, compiler library identity, analyser revision, and artefact digest.
   Resolve the then-current pins again when executing the experiment.
2. Establish a successful ordinary build/check for the same selected production
   configuration. Preserve the toolchain, Polonius tags, trait-solver contract,
   dependencies, and production implementation. Do not cure analyser problems by
   cloning values, replacing borrow-returning APIs, disabling relevant features,
   or adding old-compiler workarounds.
3. Check installation and compiler-driver compatibility, including matching
   compiler-development components. Distinguish the tool's build compiler from
   the compiler that actually analyses Netsuke; a `cargo +...` spelling alone
   does not establish which compiler the plugin uses.
4. Run extraction through an actual production entry point with declared
   expected roots and symbols. Require a non-empty graph and evidence that the
   intended production path is present. Compiling the analyser or a toy example
   is not a pass. Record crashes, unsupported constructs, warnings, and flags.
5. Repeat the extraction with the provisioned bundle to distinguish a working
   reproducible setup from a one-off local environment accident.

A pass is **compatible for the recorded production configuration**, not all
Netsuke code or platforms. Instrumentation-only annotations may identify the
path, but the implementation must remain the production implementation. A shared
production package may support a narrower scoped result; copied, rewritten, or
behaviourally substituted demonstration code cannot establish Netsuke
compatibility. Analyse every configuration later claimed by a policy.

Classify the outcome as compatible, incompatible, or inconclusive. Incompatible
and inconclusive outcomes stop later gates and retain evidence. Record whether
an upstream port could address the blocker, but any analyser port needs a
separate owner, bounded proposal, and approval. Do not make such a port an
implicit dependency of Netsuke hardening. Never downgrade Netsuke to Paralegal's
older compiler or accept a substituted analysis compiler as the P0 pass.

### P1: Semantic checks must add distinguishable value

After P0 passes, qualify at most two policies against real production paths:

- **NSFLOW001, recipe resolution:** every relevant data-dependence path from
  authored recipe payload to emitted executable payload crosses successful
  construction of the resolved operation. This policy waits for phase 26's
  resolved-operation boundary. It must not prohibit legitimate manifest-derived
  target names or provenance, nor mark an entire context as a checkpoint.
- **NSFLOW002, redirect transition:** each dispatched redirect target derives
  from the accepted policy transition for that same target. Analyse the actual
  stateful implementation first. An initial request needs its own checked-entry
  obligation; a redirect-only result must declare that limitation.

For each policy, freeze at least six violating variants and three legitimate
variants before tuning the query. Violations should cover a bypass, an ignored
result, an incorrect branch where applicable, approval of one value followed by
use of another, a legacy route, and marker/checkpoint weakening. Keep at least
two additional reviewer-selected violating variants per policy as a hold-out
set. All variants must be plausible changes to the production path, not just
examples tailored to the analyser's demonstration API.

Require detection of every applicable seeded violation and acceptance of every
legitimate control, without adding exceptions for those controls. The coverage
gate, rather than a flow query, may detect removed markers. Report which layer
caught each defect. Compare the same variants with types, structural rules, and
existing tests: continued investment requires at least one non-redundant,
reviewer-confirmed architectural regression caught by each retained policy.
Otherwise record that the simpler mechanisms suffice and stop that policy.

Dependency does not prove value equality, correct branch polarity, correct
validation, or concurrency ordering. A data-path checkpoint must identify the
successful value-producing operation, not merely entry to a function called
`validate`. Preserve type-level shell binding and accepted-value ownership.
Qualify the exact upstream query semantics rather than relying on names such as
`always_happens_before` to imply execution order.

### P2: Coverage, reproducibility, and cost must be acceptable

Inventory expected roots, sources, sinks, and checkpoints independently of the
query. A missing consumer, empty root set, vacuous query, analysis timeout,
unsupported construct, stale graph, or incomplete extraction is not success.
Compare expected symbols and annotations with the extracted graph. Do not rely
solely on the analyser's default warning behaviour or a no-violations result.

Hash the analysed source and dependency/configuration inputs. Cached graphs must
match that identity. Cache keys include analyser and compiler identities,
lockfile, target, features, policy/marker configuration, and extraction options.
A policy-only rerun may reuse a graph only when its recorded extraction inputs
remain valid. Policy or approximation changes require fresh qualification.

Use proposed ceilings of 4 vCPU and 8 GiB for build/analysis and 1 vCPU and 2
GiB for non-build report checks. Bound P0 provisioning plus its first extraction
to 30 minutes of runner wall time; stop with an inconclusive result when the
budget expires. Separately measure provisioning, compilation, extraction, policy
evaluation, and report processing. No unbounded retry loop is permitted.

For a qualified, provisioned policy suite, target at most 15 minutes cold and 3
minutes warm per run, within the same memory ceiling. Record one cold run and
three warm runs, including every failure, cache state, peak resident memory,
wall time, CPU time, retained artefact/cache bytes, runner allocation, and
billed usage at the rate in force during the trial. These are pilot ceilings,
not scheduling commitments. A reviewer must approve any revised ceiling before
another run; changing a threshold after seeing a failure is not a pass.

Prefer a vetted pinned binary when available. Otherwise provision a cached,
versioned analyser/toolchain bundle in a separately bounded build, recording
provenance and integrity. Do not run an unpinned remote installer or rebuild the
analyser in every pull request. Separate provisioning cost from marginal check
cost, but include both in the adoption assessment. Add no recurring job or
larger runner as an incidental experiment change.

### P3: Make an explicit retain, defer, or reject decision

Only policies passing P0 to P2 enter non-blocking shadow evaluation on ten
distinct, representative revisions or reviewed replay changes. Include relevant
boundary modifications and benign refactors, not ten reruns of unchanged code.
Triage every diagnostic against an independently reviewed expected result and
report exceptions, analysis failures, and maintenance effort. Do not generalize
the small sample into a universal false-positive rate.

Retain a policy only with complete declared coverage, all negative controls
caught, no unexplained control-set failures, reproducible execution within the
approved budget, and demonstrated benefit beyond the other checks. A later,
separate acceptance decision may make a retained policy blocking for its
qualified scope. This RFC never makes Paralegal mandatory automatically.

Defer when a bounded upstream/toolchain or precision improvement could justify
another trial; state a concrete revisit trigger. Reject when the supported
scope, maintenance, incremental value, or cost is unsuitable. Remove unused
instrumentation and experimental dependencies after a defer/reject outcome;
retain the result and fixtures needed to explain the decision. Do not create an
indefinite monitoring obligation.

## Evidence and governance contract

Each result records the gate and outcome; source, compiler, analyser, and policy
identities; command and environment/configuration inputs; roots and expected
symbols; coverage and unsupported cases; fixture and hold-out results; all
measurements; artefact digests; exceptions; reviewer; and decision rationale.
Use explicit statuses for satisfied, violated, exempted, not applicable by
policy, and incomplete. Not applicable requires a reviewed reason.

Marker meanings, constructors, analysis roots, approximation settings, trusted
summaries, exclusions, and CI invocation are architectural review surfaces. An
agent must not repair a failure by broadening a checkpoint or deleting its
consumer. Reuse RFC 0027's exception and coverage mechanisms where appropriate;
do not create an independent allow-list with weaker governance.

Paralegal cannot by itself establish what an external shell command does or
which bytes Ninja later reads through a pathname. Keep filesystem publication,
lease lifetime, process behaviour, and validation correctness in types and
contract tests. Unsafe code, foreign-function interfaces, interior mutability,
opaque calls, and external effects need explicit per-policy qualification. A
dependence-graph result is scoped evidence under documented approximations, not
a whole-program proof.

## Alternatives considered

Types, structural checks, and contract tests remain the baseline. Immediate
mandatory Paralegal adoption would bet governance on untested compatibility and
precision. A toy-only demonstration answers a different question. Downgrading
Netsuke would sacrifice an accepted compiler contract for an optional analyser.
An open-ended upstream port would turn a bounded experiment into another
product.

## Recommendation

Authorize only the bounded P0 preflight initially. Proceed through later gates
only on recorded evidence. A useful negative result closes the experiment while
Netsuke's architectural hardening continues.
