# RFC 0022: Optional typed task inputs

## Preamble

- **RFC number:** 0022
- **Status:** Proposed
- **Created:** 2026-09-19
- **Scope:** Public task configuration with gradual adoption
- **Implementation:** [Progressive-enhancement roadmap, phase 21][roadmap]

## 1. Summary

Add an optional `inputs` mapping for validated, discoverable task
configuration. Ordinary `vars` remain valid and keep their current semantics.
An author can promote one externally meaningful value without annotating every
variable, rewriting every command, adopting bundles, or declaring an execution
context.

Use the same type vocabulary, value validation, and redaction contract as
[RFC 0003's bundle parameters][bundles]. Do not create a rival parameter
system. The [maturity-policy RFC][maturity] permits explicit organizations or
projects to require selected contracts; it does not turn annotations into a
default gate.

## 2. Problem and current boundaries

Raw flag strings combine user intent, argument transport, and tool-specific
syntax. The distinction matters when compilation workers and test workers use
different switches. Validation should reject an invalid worker count before any
command starts, while argv handling should remain RFC 0001's responsibility.

The current manifest supports variables. RFC 0003 already proposes typed bundle
parameters, and roadmap phase 5 owns profile and context integration with
OrthoConfig. This RFC supplies task-input semantics and their integration, not
another configuration loader, profile store, or command metadata generator.

## 3. The shallow end and one-value promotion

This existing-style fragment remains legitimate without annotation:

```yaml
vars:
  workers: 2

actions:
  - name: test
    command: "pytest -n {{ workers }}"
```

The following proposed fragment adds a contract for that one input:

```yaml
inputs:
  workers:
    type: integer
    minimum: 1
    default: 2
    description: Number of pytest workers
    expose: non-secret

actions:
  - name: test
    command:
      invoke: pytest -n {{ inputs.workers }}
```

Changing `workers` to `inputs.workers` makes the promotion explicit. There is
no implicit alias, mutation of `vars`, or requirement to annotate unrelated
values. Structured invocation is recommended for dynamic arguments, but
adopting an input does not silently convert a legacy shell string into direct
execution.

## 4. Definition and value model

The initial `type` vocabulary is exactly `string`, `bool`, `integer`, `path`,
`sequence<string>`, `sequence<path>`, and `mapping<string, string>`. A string
with `choices` represents an enum. An argument list uses `sequence<string>`;
there is no competing `argv` or `enum` type in the initial grammar.

A definition contains required `type`, optional `default`, `description`, and
`expose`, plus applicable constraints. No default means required. `minimum` and
`maximum` constrain integers; `choices` contains a nonempty, duplicate-free
list of values of the declared scalar type. Reject inverted ranges, mismatched
constraints, unknown fields, and defaults that violate their own contract.

Definitions and defaults are literal data, like bundle descriptors, not Jinja
programs. Computed internal values belong in ordinary `vars` after input
resolution. No executable, clock, filesystem, or network default expressions
are necessary. Required but unused root inputs still fail definition
resolution; authoring an optional facility must not introduce an unrelated
required input.

Use the existing supported integer range and reject out-of-range values. A
Boolean is not an integer; `1.0` is not silently accepted as `1`. Strings
remain strings, including empty strings. Null is not an omitted optional value
unless a future shared nullable-type contract explicitly introduces it.

`path` is a typed path value, not a filesystem capability or permission grant.
Validation checks encoding and lexical shape without creating files. A consumer
that opens, executes, or deletes the path applies its own workspace and
operator capability constraints at use time. Collections are homogeneous,
bounded, and validated element by element; sequence order is significant.

`inputs` is immutable during manifest expansion. Introducing this reserved
namespace must preserve manifests that do not opt into the new schema. When an
opted-in manifest already defines a conflicting `vars.inputs`, emit a targeted
migration diagnostic rather than overwrite it silently.

## 5. Sources, precedence, and provenance

Introduce one proposed CLI option, `--input NAME=VALUE`, on commands that
compile a manifest. Register it through the canonical command metadata before
public examples or implementation. Names select declared root inputs only.
Qualified private bundle parameters remain inaccessible; the root passes
selected values through the existing `with` boundary.

CLI parsing is type-directed, not shell splitting or arbitrary YAML evaluation:

- Strings and paths retain the text after the first `=` exactly.
- Integers use the documented signed decimal grammar without floats.
- Booleans accept exactly `true` and `false`.
- Sequences and mappings accept JSON of the declared shape, with bounded size
  and explicit duplicate-key rejection.

Reject unknown names and duplicate CLI occurrences. Do not interpret an input
as an executable shell fragment. Direct argv interpolation must preserve
spaces, metacharacters, empty elements, and sequence boundaries. The shell
still owns interpretation inside an explicitly selected shell recipe.

Precedence, highest first, is explicit CLI input, explicitly selected profile
input, resolved configuration input, then manifest default. Existing
OrthoConfig rules determine precedence within configuration sources; this RFC
does not reorder system, user, project, and explicitly selected configuration
files. The profile integration task must reconcile its own overlay order with
this contract before implementation. No automatically inferred environment
variables supply task inputs in the first version.

Validate the effective value once, retaining the winning source and bounded
shadowed-source provenance. Duplicate declarations and malformed source data
remain errors even when a higher-precedence value exists. Configuration syntax
cannot create undeclared inputs or weaken their constraints. Operator ceilings
apply after selection and may reject a request; they are not defaults that a
project can override.

Profiles bind input values; they do not replace input definitions. Inspection
must identify the effective profile, source, and validation contract without
running a build or acquiring tool environments. Persistent generated plans fix
their resolved values: replay never re-reads a different ambient profile.

## 6. Composition and identity

Includes follow RFC 0002's duplicate and provenance rules. The initial root
`inputs` mapping supplies the root interface. Included fragments can reference
it within their established scope; they cannot silently overwrite definitions.
A bundle receives explicit values through `with` and exposes them internally as
`bundle.params`, not the importer's whole `inputs` object.

Extract or reuse one feature-owned normalized parameter contract for root
inputs and bundle parameters. Share parsing, validation, scalar normalization,
constraint diagnostics, and redaction. Bundle selection, locks, private
exports, and namespace resolution remain composition's responsibility. Root
input support must not wait for external acquisition or require any bundle.

Resolved values used by an action or state contribute to its existing
fingerprint. Preserve collection order and canonicalize mapping-key order. Do
not replace action hashing or invent a separate build cache. Unused inputs must
not alter a resolved command's argv; conservative graph-level invalidation may
remain until dependency tracking can narrow it safely.

Raw argument-list escape hatches remain legal. Tool adapters that claim control
over workers, interpreter selection, or environment location must reject
conflicting owned switches in those lists. Typed data alone does not guarantee
that a tool-specific flag bag respects the declared policy.

## 7. Diagnostics, redaction, and limits

Follow RFC 0003: values are redacted by default. Only the exact declaration
`expose: non-secret` permits ordinary inspection of a value, subject to
stronger operator policy. Show the name, expected type, constraint, source
location, and remedy without echoing a rejected secret-looking value. Human
output, JSON, verbose output, snapshots, and telemetry share that boundary.

Redaction is not secret storage. This RFC does not introduce a secret type or
promise confidentiality for values deliberately passed as process arguments or
persisted in action plans. Future secret support must address process listings,
plan storage, and low-entropy digest leakage rather than merely hide UI text.

Use the existing evaluation budgets for definition count, collection depth,
element count, input byte size, and diagnostics. Apply bounds before expensive
allocation or evaluation. A maturity setting cannot disable type correctness,
resource limits, or capability checks for an opted-in declaration.

## 8. Compatibility, tests, and rollout

Allocate the manifest version with the shared schema acceptance work. Older
readers must reject new syntax with a useful version remedy rather than ignore
it. Existing untyped manifests keep their parse, execution, and diagnostic
behaviour under the default policy. No release should force a broad migration
merely because this optional vocabulary exists.

Test the same value corpus through bundle and task-input validation. Include
false-as-integer, overflow, whitespace, empty strings, duplicate keys, invalid
constraints, required values, unknown names, hostile argv elements, Windows
paths, and collection-order preservation. Property-test source precedence,
normalization stability, and redaction of rejected values.

Integration tests must cover profile selection, generated-plan replay,
namespace privacy, environment-independent defaults, and values passed across
an include/bundle boundary. Check one-variable promotion in a real command and
retain the unmodified hello-world fixture. Compare actual child argv, not just
rendered YAML. Add examples and metadata through the existing documentation and
schema pipelines, without a parallel input-help renderer.

## 9. Alternatives and outstanding decisions

Mandatory annotations would obstruct onboarding. Continuing with only raw flag
strings would leave validation and discovery to each recipe. A second schema
language for task inputs would duplicate RFC 0003 and make composition harder.
Implicit promotion of every variable would unexpectedly create a public API.

Before implementation, ratify the CLI option's placement in canonical metadata,
profile overlay details, and the exact existing integer and path-normalization
contracts to share. Whether richer dependent defaults or custom validators are
needed remains deferred until canaries demonstrate demand. Arbitrary validation
code is not part of the initial input schema.

## 10. Recommendation

Deliver one optional root interface with the existing bundle type vocabulary,
explicit promotion, deterministic source selection, and default redaction. Keep
internal variables untyped unless their author chooses otherwise.

[roadmap]: ../roadmap-progressive-enhancement.md#21-typed-inputs-with-one-parameter-contract
[bundles]: 0003-versioned-local-bundles.md#6-parameter-model
[maturity]: 0025-progressive-enhancement-and-maturity-policies.md
