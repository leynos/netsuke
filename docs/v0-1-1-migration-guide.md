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
