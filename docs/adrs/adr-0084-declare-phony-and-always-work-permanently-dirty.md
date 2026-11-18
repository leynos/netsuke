# Architectural decision record (ADR) 0084: Declare phony and always work permanently dirty to Ninja

## Status

Accepted on 2026-11-17. An executable `phony` edge is published to Ninja as a
native `phony` alias over a hidden content-addressed output, and both executable
`phony` edges and non-phony `always` edges depend on one shared, permanently
dirty `phony` node.

## Date

2026-11-17.

## Context and problem statement

Netsuke lowers each executable manifest entry to one Ninja `build` statement
whose left-hand side is the entry's declared name. That is correct for a target
that produces a file, and wrong for two shapes that do not produce one:

- An entry in `actions`, and any target with `phony: true`, declares a name that
  is not a file. Ninja cannot know that. Once a file or directory with the same
  name exists — a `test/` directory beside an action named `test`, or any path
  the recipe itself writes — Ninja compares the name's modification time
  against the inputs, decides the edge is up to date, and never runs the recipe
  again. `netsuke clean` has the mirror-image defect: the same name is a
  declared output, so `ninja -t clean` removes the user's file or directory.
- A target with `always: true` declares that its recipe runs on every
  invocation. The emitter expressed that as an edge-level `restat = 1`, which
  asks Ninja to re-stat the outputs after a run; it does not make the edge
  dirty. An `always` target whose output exists and whose inputs are unchanged
  was therefore skipped.

Both defects are confined to the Ninja backend. The IR already carries the two
flags (`BuildEdge::phony` and `BuildEdge::always`), and the graph identities,
manifest schema, help output, defaults selection, and determinism contract are
unaffected. Dependency-only lowering — `build all: phony | dep` — was already
correct and is untouched.

## Decision drivers

- A manifest name for an action must not acquire filesystem meaning: neither an
  existing path nor a path the recipe writes may suppress the recipe.
- `netsuke clean` must never delete a user's file or directory because its name
  happens to match a logical target.
- `always: true` must actually re-run the recipe, while an unchanged output must
  still prune downstream work.
- The IR, public APIs, and manifest schema stay unchanged; the decision belongs
  to the backend.
- The string generator and the serial-dependency bundle must emit identical
  semantics.
- Graphs that use neither flag keep their previous bytes.

## Options considered

### Option A: delete a colliding path before the recipe runs

Netsuke would remove whatever occupies the declared name before invoking Ninja.
This destroys user data, and it cannot be expressed in a static Ninja file
anyway: a Ninja `command` runs only when the edge is already considered dirty,
which is the situation the collision defeats.

### Option B: hide the recipe behind a content-addressed output and publish the name as an alias

The executable edge's output becomes a generated path under `.netsuke/phony`,
and the manifest name becomes a native `phony` alias over it. The recipe's
dirtiness is then decided by a path no user can occupy, and `clean` never
touches the manifest name.

### Option C: keep the public output and add a permanently dirty prerequisite

`build out: rule in | force` with `build force: phony` makes the edge run every
time. This is sufficient for `always`, and insufficient for `phony`: the name
is still a produced file, so a same-named directory remains an "up to date"
output and is still removed by `clean`.

### Option D: retarget the manifest name to the hidden path

Netsuke would generate a hidden output only and never mention the manifest name
in the generated file. Users name targets by manifest name on the command line,
in `defaults`, and in diagnostics, so the generated file must keep publishing
that name.

### Option E: lower `always` to a rule-less phony node

Emitting `build out: phony` discards the declared output, and with it the
freshness contract every dependant's rebuild decision depends on.

| Topic                      | B: hidden output plus alias        | C: force prerequisite only |
| -------------------------- | ---------------------------------- | -------------------------- |
| Same-named directory       | name is an alias, never up to date | still treated as an output |
| `clean` removes user path  | no                                 | yes                        |
| Recipe re-runs every build | yes                                | yes                        |
| Suited to `always`         | no (discards the real output)      | yes                        |

_Table 1: The two options that survived initial review, against the two
defects._

## Decision outcome

The two flags are lowered differently, because they promise different things.
Both share one force node.

### Executable `phony` edges

The recipe moves to a hidden output whose name is derived from the edge's own
outputs, and the declared names become a native Ninja `phony` alias:

```ninja
rule check
  command = ...

build .netsuke/phony/force: phony

build .netsuke/phony/edge-<digest>: check in | .netsuke/phony/force
build check: phony .netsuke/phony/edge-<digest>
```

The hidden output is the edge's only produced path, so no user path can make it
look up to date, and `ninja -t clean` — which skips `phony` outputs — leaves a
same-named user path alone. Progress metadata (`description`, `pool`) stays on
the hidden statement, which is the statement that runs the recipe. `restat` is
suppressed there: the hidden path is never written, so a restat claim on it
could not prune anything.

Implicit outputs join the alias, `build out | out.d: phony <hidden>`, and stay
off the hidden statement.

### Non-phony `always` edges

The declared output is real, so it stays the recipe statement's output and the
edge gains the force node as an implicit dependency:

```ninja
build out: check in | .netsuke/phony/force
  restat = 1
```

The edge-level `restat = 1` is kept exactly as before. It is what tells Ninja
to compare the output's modification time after the run, so a recipe that
rewrites an unchanged file still prunes its dependants.

### Shared details

- `.netsuke/phony/force` is declared once, as
  `build .netsuke/phony/force: phony`, and only when a rendered edge consumes
  it.
- The hidden identity hashes the explicit outputs and then the implicit
  outputs, NUL-separated within a group and separated by `U+0001` between
  groups, under the schema tag `netsuke-phony-v1`. Path validation already
  rejects `:`, `|`, `$`, and every control character, so the encoding is
  unambiguous and insertion order cannot change the digest.
- `.netsuke/phony` is reserved, alongside `.netsuke/serial` and
  `.netsuke/dyndep`: a user graph path inside it is rejected before generation,
  so a manifest cannot forge or collide with a hidden output.
- The `edge-` prefix keeps the force node from being a parent path of any hidden
  output.
- Dependency-only aggregates keep their rule-less `build all: phony | dep` form,
  whatever their own `phony` flag says.
- Neither the force node nor a hidden output is added to the default targets.

## Goals and non-goals

- Goals:
  - An action or `phony` target runs on every request, whatever occupies its
    name.
  - `always` re-runs a file-producing target whose output is up to date.
  - Unchanged output from an `always` recipe still prunes downstream edges.
  - One lowering serves the string generator and the serial bundle.
- Non-goals:
  - Changing `BuildEdge`, the graph, the manifest schema, or any public API.
  - Changing dependency-only lowering.
  - Renumbering or rewriting generated paths outside `.netsuke/phony`.

## Migration plan

No manifest change is required. Existing manifests gain corrected behaviour:

- An action or `phony: true` target that a same-named path used to suppress now
  runs on every `netsuke build`.
- `netsuke clean` no longer removes a user path whose name matches such an
  entry, and now removes the generated `.netsuke/phony/edge-*` outputs instead.
- An `always: true` target that used to be skipped when up to date now re-runs.
  Its dependants still rebuild only when the recipe changes the output.
- Generated Ninja text changes shape for these edges. The determinism guarantee
  is unchanged: for one graph, environment, platform, shell selection, and
  Netsuke version, the emitted bundle is still byte-identical. Manifests that
  assert the old text in their own checks must update those assertions.

## Known risks and limitations

- A `phony` edge's declared name no longer names a file Ninja can stat. Anything
  that inspects the generated file for modification times of an action name
  must interpret the alias instead; users interact with the manifest name, not
  the generated statement.
- The localized reserved-namespace message still names only the serial
  dependency state. The users' guide and this record name all three namespaces
  precisely, and the catalogue text is left for a dedicated translation change.
- `netsuke clean` removes the hidden outputs but not the now-empty
  `.netsuke/phony` directory.
- Ninja creates the `.netsuke/phony` directory when it first runs a hidden
  recipe, so a build that skips every such edge never creates it.

## Architectural rationale

The change keeps the backend's job as a mechanical translation of the IR, and
moves the difference into the statement shape rather than into new IR state:
the flags already existed and were previously ignored or overloaded. It also
keeps the generator's determinism claim intact, because every generated path is
either derived from the graph's own paths or a fixed reserved name.
