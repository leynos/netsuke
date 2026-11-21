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
time: while the force node is absent, Ninja treats its dependant as dirty
through that implicit input even when the declared output already exists. This
is sufficient for `always`, and insufficient for `phony`: the name is still a
produced file, so `clean` removes it, including a user's same-named file or
directory when one occupies that path.

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
suppressed there, and a rule-level `restat = 1` the action sets is cleared with
an empty `restat =` binding: an edge inherits its rule's bindings, and the
hidden path is never written, so a surviving `restat` claim could only prune
dependants whose work was never up to date.

Implicit outputs join the alias, `build out | out.d: phony <hidden>`, and stay
off the hidden statement.

### Non-phony `always` edges

The declared output is real, so it stays the recipe statement's output and the
edge gains the force node as an implicit dependency:

```ninja
build out: check in | .netsuke/phony/force
  restat = 1
```

The edge-level `restat = 1` is kept exactly as before. Ninja records each
output's modification time after the run and treats an output whose time did
not change as clean, so a recipe that leaves the output's time alone prunes its
dependants. A rewrite rebuilds them only when it changes the recorded
modification time; a rewrite that leaves that time unchanged still prunes them.

### Shared details

- `.netsuke/phony/force` is declared once, as
  `build .netsuke/phony/force: phony`, and only when a rendered edge consumes
  it.
- `netsuke build` removes any path left at `.netsuke/phony/force` before it
  invokes Ninja: the node is dirty only while that path is absent, and the
  removal belongs to the command that owns build state rather than to the pure
  generator.
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
- Ninja creates the parent directories of the outputs on an edge it is about to
  run. Before the lowering, that covered a recipe writing a nested output such
  as `build/report`; afterwards the only output Ninja sees is the hidden path,
  so generation emits a preparation edge for each distinct declared-output
  parent, and the recipe statement names those nodes after `||` as order-only
  dependencies. Order-only keeps a missing directory from making the recipe
  permanently dirty on its own; the preparation edge itself takes the recipe's
  `inputs`, `implicit_deps` and `order_only_deps` as its normal inputs, because
  Ninja does not order an order-only dependency after the edge's other
  prerequisites, and a prerequisite that creates the same directory with a plain
  `mkdir` must still run first. Each node's identity is content-addressed over
  the edge's hidden output and the directory, so two recipes sharing a
  directory share one rule and keep distinct nodes. The node is never created,
  which Ninja reads as permanently out of date: the idempotent command reruns
  whenever the statement is selected, so a directory deleted since the last
  build returns, and neither a missing-output warning nor a `clean` entry
  results. An unselected edge prepares nothing, and an output whose parent is
  the working directory emits no statement at all. `ninja_gen::edge_metadata`
  answers which edges are lowered, because it owns that classification, and the
  emitted command goes through the same shell and interpreter transport as a
  manifest recipe, so quoting and PowerShell encoding keep one owner.

  _Amendment (2026-11-21)._ Preparation first lived in the runner's command
  layer: it created every executable phony edge's parent directories before
  invoking Ninja. Review found two consequences of that placement. It ran for
  unselected edges, so a manifest naming `optional/report` where `optional` was
  a regular file failed an unrelated build before Ninja started. And it ran
  before the edge's prerequisites, so a prerequisite that created the same
  directory with a plain `mkdir` failed with `File exists` once Ninja reached
  it. Generating the preparation as an ordinary edge fixes both: it is reached
  only through the selected recipe statement and runs after that recipe's
  prerequisites. Generation remains a pure translation of the graph, because a
  generated statement is text like any other, so the command layer needs no
  filesystem access of its own and the runner-side Whitaker exclusion is gone.
  Real-Ninja tests pin the nested-output case, the unselected case, the
  prerequisite case, and the recreation of a deleted directory.

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

One manifest shape must change: a manifest that declares a path under
`.netsuke/phony` must move that path, because the namespace now holds the state
these lowerings generate and generation rejects the spelling. Every other
manifest gains corrected behaviour without an edit:

- An action or `phony: true` target that a same-named path used to suppress now
  runs every time the target is reached.
- `netsuke clean` no longer removes a user path whose name matches an
  executable phony entry, and now removes the generated `.netsuke/phony/edge-*`
  outputs instead.
- An `always: true` target that used to be skipped when up to date now re-runs.
  Its dependants still rebuild only when the recipe changes the output's
  modification time.
- A path an older Netsuke run declared as an executable phony edge's output is
  no longer declared, so `netsuke clean` will not remove it. A recipe that
  still writes its public name recreates that path on every build, so a single
  removal is not enough: the stale product needs a removal step separate from
  `netsuke clean`, repeated on every run until the recipe stops writing that
  name. A recipe that has stopped writing it leaves one stale copy, which a
  single removal clears.

  _Addendum (2026-11-20)._ The original text advised deleting the stale build
  product once. That holds only for a recipe that no longer writes its public
  name. A recipe that still writes it — including one that redirects into the
  declared file, whether it names that path directly or through the
  `{{ outs }}` marker, which is lowered from the declared names — recreates the
  path on every build, outside Ninja's knowledge, so a step separate from
  `netsuke clean` has to remove it on every run.
- Generated Ninja text changes shape for these edges. The determinism guarantee
  is unchanged: for one graph, environment, platform, shell selection, and
  Netsuke version, the emitted bundle is still byte-identical. Manifests that
  assert the old text in their own checks must update those assertions.

## Known risks and limitations

- A `phony` edge's declared name no longer names a file Ninja can stat. Anything
  that inspects the generated file for modification times of an action name
  must interpret the alias instead; users interact with the manifest name, not
  the generated statement.
- The English reserved-namespace message now names generated build state rather
  than serial dependency state, which covers all three namespaces. The other
  catalogues retain the earlier wording until a dedicated translation change,
  so their readers still see a message written for the serial namespace alone.
  The users' guide and this record name all three namespaces precisely.
- `netsuke clean` removes the hidden outputs but not the now-empty
  `.netsuke/phony` directory.
- Ninja creates the `.netsuke/phony` directory when it first runs a hidden
  recipe, so a build that skips every such edge never creates it.
- A preparation node is never created, so an edge that reaches it re-runs its
  idempotent `mkdir` on every build. That is the same mechanism the lowering
  relies on elsewhere, and the command is chosen to be safe to repeat; the cost
  is one extra shell per selected nested phony output, per build.
- A `depfile` on an executable phony edge is a rule binding, so the hidden
  statement inherits it. Without a `deps` binding, Ninja reads the depfile at
  scan time and accepts it only when its first target equals the edge's first
  output, which is now the hidden path. No recipe can name that path:
  `{{ outs }}` is lowered from the declared names, and `$out` is a literal
  shell variable like every other recipe dollar, so a Makefile-style depfile
  written by the recipe is refused and orders nothing. A rule that also sets
  `deps` records the depfile's inputs in Ninja's dependency log against the
  hidden output, so they order later builds; the build that records them cannot
  use them. Real-Ninja tests pin both outcomes. The manifest loader never sets
  `Action.depfile` or `Action.deps_format` in beta4, so only programmatic IR
  construction reaches this path today.
- The force node is a real path in the build directory. If a file, directory,
  or symbolic link exists at `.netsuke/phony/force`, Ninja finds the phony
  node's output present and treats the forced edges as clean, so they stop
  re-running. `netsuke build` therefore removes a path found at that node
  before it invokes Ninja, and warns when it does; the path is inside a
  namespace reserved for generated state, so nothing a manifest declares is
  lost — provided the manifest does not name that node indirectly. Generation
  rejects the literal and foldable spellings, but an absolute path, a `..` path
  that leaves the working directory and returns, and a symbolic-link alias can
  only be judged against that directory. The build therefore resolves every
  declared path before it deletes anything and refuses the reset when one
  resolves into a reserved namespace, so a spelling that names a force node its
  manifest also declared is refused rather than destroyed. Resolution only
  reads; the removal itself still goes through a verified directory handle.
  Generation continues not to inspect the filesystem, because emitting a Ninja
  file is a pure translation of the graph, so a generated file run by hand
  under a directory holding a stray force node keeps the old behaviour until
  that path is removed.

  _Amendment (2026-11-19)._ The alias check was added after review found that
  lexical comparison over the graph's spelling accepted both an absolute path
  and a parent-relative path that re-enters the workspace, either of which
  would have deleted a declared output. The generator's purity is preserved:
  the reservation rule stays with generation, and the working-directory
  resolution lives in the runner's command layer.

## Architectural rationale

The change keeps the backend's job as a mechanical translation of the IR, and
moves the difference into the statement shape rather than into new IR state:
the flags already existed and were previously ignored or overloaded. It also
keeps the generator's determinism claim intact, because every generated path is
either derived from the graph's own paths or a fixed reserved name.
