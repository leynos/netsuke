# ADR-031: Gate Paralegal adoption on measured evidence

## Status

Proposed.

## Date

2026-09-19.

## Context and problem statement

Paralegal may detect interprocedural semantic bypasses that import rules miss,
but its compatibility, precision, coverage, and cost on Netsuke are unmeasured.
Source inspection found different compiler pins: Netsuke uses
`nightly-2026-08-23`; the inspected Paralegal revision uses
`nightly-2026-04-20`. [RFC 0028][experiment] records exact revisions,
references, experimental thresholds, and the initial result of not run.

[experiment]: rfcs/0028-paralegal-architecture-experiment.md

## Decision drivers

- Preserve Netsuke's accepted Polonius and trait-solver toolchain contract.
- Require production-code evidence rather than a successful toy demonstration.
- Establish benefit beyond types, dependency rules, and existing tests.
- Distinguish unanalysed or vacuous results from satisfied policies.
- Bound provisioning, runtime, memory, and ongoing maintenance cost.

## Proposed direction

Authorize a staged experiment, not mandatory Paralegal adoption. P0 must first
establish that the analyser actually uses Netsuke's required compiler and can
extract the intended production path. Different declared pins are a risk, not a
compatibility verdict. Record actual compiler identity and execution evidence.

On incompatibility or an inconclusive bounded run, stop and record a defer or
reject decision. Do not downgrade Netsuke, rewrite production borrowing for an
older compiler, remove relevant code/features, or count a stand-in
implementation as success. A proposed upstream analyser port requires separate
approval.

Only a compatible configuration proceeds to at most two qualified policies:
recipe resolution and redirect transitions. Require independent positive and
negative controls, hold-out mutations, coverage checks, non-redundant benefit,
and measured cost. Use non-blocking shadow evaluation before a separate decision
can authorize a required check. Promotion is never automatic.

Retain types and contract tests for value equality, validation correctness,
resource lifetime, shell behaviour, and external-file effects. A dependence
query supplies scoped evidence under its approximations, not a general proof.

## Alternatives considered

Immediate adoption risks making governance depend on an incompatible analyser.
Downgrading the application sacrifices an accepted design for an optional tool.
Toy-only analysis cannot establish Netsuke coverage. An open-ended upstream port
would exceed the experiment. Rejecting the idea without a bounded trial would
forgo potentially useful evidence; P0 offers a proportionate initial test.

## Consequences and migration

Phase 29 of the [architecture roadmap](roadmap-hexagonal-hardening.md) owns the
trial. RFC 0028 supplies falsifiable criteria and bounded resource ceilings.
Provision pinned cached artefacts, prefer vetted binaries, record all failures,
and make unsupported paths explicit. No analyser installation, dependency,
marker, schedule, or CI gate is introduced by accepting this experiment plan.

A defer/reject outcome closes the experiment legitimately, retains evidence and
a concrete revisit trigger where relevant, and removes unused instrumentation.
The hardening and structural checking programmes continue independently.

## Known risks and limitations

Compiler plugin interfaces and supported Rust constructs may change. Positive
results apply only to the recorded revision/configuration and qualified policy.
Small controlled samples do not establish universal detection or false-positive
rates. Review must prevent marker weakening and missing-root false passes.

## Outstanding decisions

Approve the initial experiment ceilings and reviewer before P0. Any analyser
port, broadened scope, budget increase, or blocking-gate adoption needs a
separate recorded decision supported by new evidence.
