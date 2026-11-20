# Migrating to v0.1.1

Netsuke v0.1.1 keeps every v0.1.0 manifest compatible and removes one
declarative-manifest paper cut: an action or target whose dependencies are its
entire operation no longer needs a shell no-op recipe.

## Replace no-op aggregate recipes

When an aggregate action exists only to group or order dependencies, remove
`command: ":"`. Leave its non-empty `deps` list and any
`dependency_order: serial` policy unchanged:

```yaml
actions:
  - name: all
    dependency_order: serial
    deps:
      - check-fmt
      - lint
      - test
```

Netsuke lowers this entry to a native Ninja `phony` node. The dependencies
retain their previous ordering, deduplication, and failure-propagation
behaviour, but the aggregate no longer launches a shell command.

Generated bundles for serial lists with two or more dependencies also contain
one relay phony for each staged dependency. Each relay carries the
corresponding real dependency; its dyndep sidecar reveals that relay as an
input to the staged gate. These relays are distinct from the dependency-only
aggregate phony such as `all`, which still represents the authored workflow.
Manifests remain compatible and need no edit for this generated-state change.
See
[ADR-0058](adrs/adr-0058-reveal-a-relay-phony-to-avoid-the-ninja-out-edge-reallocation-crash.md)
for the reason for this indirection.

An entry with neither a recipe nor a non-empty `deps` list remains invalid.
Continue using `command`, `script`, or `rule` whenever the action or target has
work of its own to perform. See the
[users' guide](users-guide.md#targets-inputs-and-dependencies) for the manifest
contract and
[serial dependency ordering](users-guide.md#run-direct-dependencies-serially)
for ordered aggregates.

## Update fixed-label queries for the `which` counters

The two `which` resolver counters keep their names, but each now carries an
extra `cwd_mode` label. `netsuke_stdlib_which_cache_total` and
`netsuke_stdlib_which_resolution_total` therefore report new series, and a
scraper, recording rule, dashboard, alert, or saved query that matched either
metric by a fixed label set must be updated.

The label is drawn from a closed vocabulary of `auto`, `always`, `never`, and
`workspace_recursive`. A manifest writes `cwd_mode="workspace-recursive"`, with
the hyphen, while the label value is the underscore spelling
`workspace_recursive`. Querying the hyphenated spelling matches nothing.

See the [users' guide](users-guide.md#which-resolver-observability) for the
counter and label reference and
[ADR-0024](adrs/adr-0024-require-explicit-recursive-which-search.md) for the
decision that added the label.

## Expect phony and always work to run

No manifest needs an edit for these two shapes, but they now behave as their
names promised. One reservation is new: a manifest that already declares a
target or output inside `.netsuke/phony` must move it, because that spelling
now names generated state and generation rejects it.

An action or target declaring `phony: true` with a recipe runs every time it is
reached, even when a file or directory of that name exists. Previously such a
path made the edge look up to date, so the recipe was skipped after its first
run. `netsuke clean` now leaves that path alone and removes the generated
output under `.netsuke/phony` instead. A dependency-only aggregate has no
recipe and no generated recipe output; it keeps its dependency-only `phony`
form whether or not it declares the flag.

A recipe that previously wrote a file at its declared name left that file
behind as the edge's declared output. `netsuke clean` no longer removes it, and
will not remove it after the upgrade either, because the generated
`.netsuke/phony` output does not replace the old path. Identify stale build
products from earlier runs and delete only the ones you have confirmed the
recipe created. A recipe that still writes its declared name recreates it on
every build, so it has to be removed on every run rather than once. Do not
delete unrelated files that merely share a target's name.

A target declaring `always: true` re-runs on every invocation that reaches it,
even when its inputs and output are unchanged. Ninja records the output's
modification time after the run: an otherwise up-to-date dependant rebuilds
when that recorded time changes and is skipped when it does not. A recipe that
rewrites identical bytes and restores the original modification time therefore
still lets `restat` skip such a dependant, while a dependant with another
changed input can still run.

Because the generated Ninja text changes shape for these edges, a manifest that
compares `netsuke generate` output against a stored copy must update that copy.
See the [users' guide](users-guide.md#rely-on-phony-and-always-work-running) and
[ADR-0084](adrs/adr-0084-declare-phony-and-always-work-permanently-dirty.md).
