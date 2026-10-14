# `v0-6-0-migration-guide.md` compatibility path

This path exists only so that relative links written against the upstream
`leynos/rstest-bdd` layout keep working after import.

The document itself is imported into Netsuke under its repository-prefixed name:

- [`rstest-bdd-v0-6-0-migration-guide.md`](rstest-bdd-v0-6-0-migration-guide.md)
  — the `rstest-bdd` 0.6.0 migration guide, imported byte-for-byte from tag
  `v0.6.0` at commit `72fb2263`.

The imported users' guide links to `v0-6-0-migration-guide.md`, which is the
upstream filename. It targets the `#migrate-a-stateful-gpui-test` fragment, so
the heading below provides that anchor and forwards to the real section in the
imported guide.

## Migrate a stateful GPUI test

The section of that name is in the imported guide at
[Migrate a stateful GPUI test](rstest-bdd-v0-6-0-migration-guide.md#migrate-a-stateful-gpui-test).

See [`rstest-bdd-link-compatibility.md`](rstest-bdd-link-compatibility.md) for
the full mapping, including the upstream files Netsuke does not carry.
