# `rstest-bdd` link compatibility

Two documents under `docs/` are imported byte-for-byte from the upstream
`leynos/rstest-bdd` repository:

- [`rstest-bdd-users-guide.md`](rstest-bdd-users-guide.md), imported from
  `docs/users-guide.md`.
- [`rstest-bdd-v0-6-0-migration-guide.md`](rstest-bdd-v0-6-0-migration-guide.md),
  imported from `docs/v0-6-0-migration-guide.md`.

Both were copied from tag `v0.6.0` at commit `72fb2263`. The provenance table
and the recorded digests live in
[`contents.md`](contents.md#testing-and-quality-references) and in
[`execplans/adopt-rstest-bdd-v0-6-0.md`](execplans/adopt-rstest-bdd-v0-6-0.md).

## Why this page exists

Upstream, those two documents refer to each other and to design records that
sit beside them in the upstream `docs/` tree. Netsuke keeps different
filenames, and does not carry the upstream design records at all, so the
relative links inside the imported text have nothing local to resolve to.

The import policy is byte-for-byte fidelity: an imported document is never
edited, because editing it would invalidate the digest that proves which
upstream revision it holds. Compatibility is therefore provided *outside* the
imported files.

That constraint is the reason this page exists rather than a set of rewritten
links. Upstream reached the same conclusion for its own vendored guide, where
[ADR-014](https://github.com/leynos/rstest-bdd/blob/v0.6.0/docs/adr-014-retain-users-guide-link-validator.md)
retains a link validator and
[ADR-021](https://github.com/leynos/rstest-bdd/blob/v0.6.0/docs/adr-021-single-source-base-url-for-users-guide-links.md)
records the base URL once.

## Renamed filenames

Upstream names its migration guide `v0-6-0-migration-guide.md`. Netsuke imports
each release's migration guide under a `rstest-bdd-` prefix, matching the
earlier `rstest-bdd-v0-5-0-migration-guide.md`, so the two releases read alike
in the index. The upstream name and the Netsuke name are the same document:

- [`rstest-bdd-v0-6-0-migration-guide.md`](rstest-bdd-v0-6-0-migration-guide.md)
  is the imported `v0-6-0-migration-guide.md`.

## Upstream files not carried locally

The migration guide links to six upstream documents that Netsuke does not
import. None of them has a local counterpart, and none should be mistaken for a
Netsuke document of a similar name. Each link below points at the pinned
upstream commit:

- [`adr-006-fallible-scenario-functions.md`](https://github.com/leynos/rstest-bdd/blob/v0.6.0/docs/adr-006-fallible-scenario-functions.md)
  — fallible scenario functions.
- [`adr-007-harness-context-injection.md`](https://github.com/leynos/rstest-bdd/blob/v0.6.0/docs/adr-007-harness-context-injection.md)
  — harness context injection.
- [`adr-009-consistent-implicit-fixture-name-normalization.md`](https://github.com/leynos/rstest-bdd/blob/v0.6.0/docs/adr-009-consistent-implicit-fixture-name-normalization.md)
  — implicit fixture name normalization.
- [`adr-012-guard-based-stepcontext-borrowing.md`](https://github.com/leynos/rstest-bdd/blob/v0.6.0/docs/adr-012-guard-based-stepcontext-borrowing.md)
  — guard-based `StepContext` borrowing.
- [`rstest-bdd-design.md`](https://github.com/leynos/rstest-bdd/blob/v0.6.0/docs/rstest-bdd-design.md)
  — the upstream design document. The guide cites §§2.7.6.1–2.7.6.5 of it by
  anchored link.
- [`developers-guide.md`](https://github.com/leynos/rstest-bdd/blob/v0.6.0/docs/developers-guide.md)
  — the **upstream** developers' guide, which is a different document from
  Netsuke's [`developers-guide.md`](developers-guide.md). This is the one
  genuinely hazardous name in the set: following it in this repository lands on
  a plausible-looking but unrelated file. No compatibility path is provided for
  it, deliberately — a `docs/developers-guide.md` that resolved to anything
  other than Netsuke's own guide would be a far worse trap than a dead link.

The `adr-*` names in that list cannot collide with Netsuke's own records. This
repository files its decisions under `docs/adrs/` with a zero-padded four-digit
serial (`adr-0006-…`), while upstream uses a three-digit serial at the `docs/`
root (`adr-006-…`). The two conventions are visibly distinct, so a reader is
not left guessing which `adr-006` is meant.

## Anchor compatibility

The migration guide links to two sections of the upstream users' guide by
anchor. Netsuke's own [`users-guide.md`](users-guide.md) is a different
document, so it cannot supply them. Netsuke's guide carries two compatibility
headings instead, each linking to the real section in the imported users' guide:

- [`users-guide.md`](users-guide.md#compatibility-links-for-the-imported-rstest-bdd-guides)
  — the compatibility section, which forwards to
  [`rstest-bdd-users-guide.md`](rstest-bdd-users-guide.md#stateful-gpui-scenarios-with-durable-handles)
  and
  [`rstest-bdd-users-guide.md`](rstest-bdd-users-guide.md#published-gpui-022-stateful-step-variants).

The imported users' guide also links to
`v0-6-0-migration-guide.md#migrate-a-stateful-gpui-test`, which resolves
through the filename mapping above to the
[`Migrate a stateful GPUI test`](rstest-bdd-v0-6-0-migration-guide.md#migrate-a-stateful-gpui-test)
section of the imported migration guide.

## Keeping this page honest

There is no automated gate on these links. If either imported document is
refreshed from a newer upstream release, re-check the relative links in the new
text before recording its digest: a freshly imported link is invisible to prose
review, and a broken one costs nothing to find now and a great deal to find
from a downstream reader.
