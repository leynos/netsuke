# Migrating to v0.2.0

The v0.2.0 release adds the `netsuke check` manifest linter. Existing manifests
need no changes: checking is read-only, and the command analyses the same
compiler artefacts as a build without running recipes or creating build output.

## Run the linter

Run the command against the selected manifest:

```sh
netsuke check
```

The linter reports likely defects, portability problems, and caching issues.
Each rule has a severity, and `--fail-on` selects the severity that makes the
command fail. Use `--rule` to change a rule or category, or to turn one off.
Suppression comments name the rules they silence and must include a reason;
there is no blanket disable. The complete rule list, policy options, and JSON
output contract are in the
[users' guide](users-guide.md#lint-a-manifest-with-netsuke-check).
