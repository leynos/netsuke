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
[ADR-024](adr-024-require-explicit-recursive-workspace-which-search.md) for the
decision that added the label.
