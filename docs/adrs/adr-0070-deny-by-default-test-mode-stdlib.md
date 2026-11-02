# Architectural decision record (ADR) 0070: Deny-by-default test-mode stdlib

## Status

Accepted on 2026-11-01. The `netsuke test` manifest loader grants a
deny-by-default stdlib surface, and template overlays replace withheld
functions by name.

## Date

2026-11-01

## Context and Problem Statement

Roadmap item 7.1.2 makes the manifest loader options-carrying so that a test
runner can load a manifest without granting the capabilities a build needs. The
technical design (§4.2, §5.5) makes three requirements binding:

- A test load must not read the host environment, inspect or mutate the project
  tree, run a command, reach the network, or depend on the wall clock. An
  unmocked impure call is an error, not a host-dependent answer (constraints
  C2, C3, C4).
- A withheld helper must fail with a located diagnostic naming the operation.
  An absent helper yields MiniJinja's generic unknown-name error, which reads
  as a typo in the manifest.
- A test author must still be able to test a manifest that calls an impure
  helper, by supplying the answer a real run would have produced without
  granting the helper itself.

The loader already selects its stdlib boundary through a crate-private
`StdlibRegistration` enum with `AmbientFull`, `Full`, and `ManifestQuery`
variants. The question is how to add a fourth mode without duplicating the
capability decision, the refusal diagnostics, or the environment seam.

## Decision Drivers

- One place decides every capability for a load (constraint C5).
- Refusals are localized, carry a stable diagnostic code, and expose a typed
  error source so a caller can distinguish a withheld capability from an
  unknown name.
- The test surface is deterministic: it depends only on inputs the test
  supplied.
- The overlay mechanism is a loader capability compiled into the shipped
  binary, not a test-only `cfg` path (constraint C9).
- Every helper whose availability differs by load surface is inventoried once,
  and a new helper cannot be registered without stating what each surface does
  with it.
- No second mode enum, and no public load-mode vocabulary that an embedder
  could widen.

## Decision Outcome

Extend `StdlibRegistration` with `Test(Box<TestSurface>)` and keep the enum
crate-private. `TestSurface` carries three things: the `StdlibConfig` the
surface grants (including the clock `now()` may read), an `EnvReader` that
`env()` answers from, and the `TemplateOverlays` the test supplied. Keeping the
payload in the surface rather than in the load options means the enum never
grows a second capability field.

Availability is decided by one inventory. `SURFACE`
(`src/stdlib/restricted_helpers/mod.rs`) records a query disposition and a test
disposition beside every helper whose availability depends on the load surface.
`register_for_test` registers exactly the refused rows as stubs, and a contract
test checks the inventory in both directions. The implemented surface is:

- Available: `env()` over the surface's injected reader; `timedelta`; the
  lexical path filters `basename`, `dirname`, `with_suffix`, `relative_to`; the
  recipe-text quoting helpers `shell_quote`, `shell_join`; the collection
  filters `uniq`, `flatten`, `compact`, `group_by`; and the legacy Boolean
  render formatter.
- Refused: the functions `glob`, `fetch`, `which`, and `command_available`;
  the filters `which`, `shell`, `grep`, `contents`, `realpath`, `expanduser`,
  `size`, `linecount`, `hash`, and `digest`; the tests `dir`, `file`, `symlink`,
  `pipe`, `block_device`, `char_device`, and `device`.
- Clock-injected: `now()` is the real helper only when the surface's
  `StdlibConfig` carries a clock supplied through `with_clock`, and a refusal
  when it would read the ambient clock.

Each refusal is a MiniJinja `InvalidOperation` error whose message is the
Fluent key `stdlib.test_mode.helper_unavailable` with argument `operation`,
prefixed by the stable code `[netsuke::jinja::test_mode::unavailable]` in every
catalogue, and whose source is a typed `TestModeRefusal { operation }`. Stubs
accept any positional and keyword arguments, because a real helper documents
several call shapes and a narrower signature would report an argument-count
error instead of the refusal it must deliver.

Two rows are deliberate deviations from a "run it against an empty sandbox"
reading, and both were reviewed before implementation:

- `which` refuses although `path_override` and `pathext_override` exist.
  Running it against an empty `PATH` reports a misleading "not found" rather
  than a refusal naming the withheld capability, and no sandbox-rooted resolver
  exists yet.
- `expanduser` refuses rather than resolving `~`. With
  `HomeDirectory::Missing` the real filter would fail anyway, and a resolved
  home path exposes a host-shaped answer.

Template overlays grant the test author a function name in the MiniJinja
environment, backed by a caller-supplied closure. Registration happens after
the stdlib, the manifest `vars`, and the manifest macros, and before `foreach`
expansion, so an overlay is visible to `when`, `foreach`, and every rendered
field and can replace a same-named helper. Overlay names are validated against
the namespaces the loader and manifest already claim — `env`, the `__netsuke_`
prefix, a `vars` key, and a manifest macro — and a collision is a typed
`TemplateOverlayError::NameCollision` rendered through
`manifest.overlay.name_collision` with code
`[netsuke::manifest::overlay::collision]`. Validation runs before any
registration, so a rejected overlay leaves the environment untouched.

MiniJinja offers no lookup by name, but registered functions are enumerable as
environment globals. Before replacing a name the loader captures the displaced
function and hands it to the overlay as `OverlayCall::shadowed`, so an overlay
can wrap a helper rather than only discard it. This is the capture a spy
depends on.

Of the refused rows, only the functions — `glob`, `which`, `fetch`,
`command_available`, and a system-clocked `now` — are replaceable by overlays
in the first version. Filters and tests are not mockable (UX design §8.6);
roadmap 7.4.1 adds sandbox-rooted adapters for `glob`, the file tests, and the
file-reading filters.

## Rationale

A fourth enum variant keeps one decision table for load capabilities. Adding a
separate `TestMode` flag, or a parallel registration path, would let two
mechanisms disagree about, for example, whether `glob` reaches the filesystem,
and a caller could combine them into a load neither designed.

The inventory-driven surface makes the deny-by-default rule mechanical. A
missed helper is a contract-test failure rather than a silent capability grant,
and the two dispositions sit side by side so query and test behaviour cannot
drift apart.

Refusing rather than omitting is the difference between a manifest author
seeing "unknown function `fetch`" (which suggests a typo) and seeing that
`fetch` is unavailable under `netsuke test` and that an overlay is the way to
answer it. The typed source keeps the distinction available to code, not just
to a human reading the message.

Composing with the existing `StdlibConfig` and `disabled_env_reader` avoids
inventing a test-only stdlib. The surface narrows what the build path already
provides and reuses its diagnostic and environment shapes.

## Consequences

- A manifest that calls a withheld helper fails under test with a located,
  localized diagnostic naming the operation; it never receives a host-dependent
  answer.
- A test can answer `glob`, `which`, `fetch`, `command_available`, and a
  system-clocked `now` with an overlay, and can wrap the displaced function
  rather than replacing it.
- Filters and tests cannot be mocked in the first version; a manifest that
  pipes through `shell` or uses `file` remains untestable until roadmap 7.4.1.
- `StdlibRegistration` stays crate-private, so the loader's four modes are not
  an embedder-facing contract.
- Query loads keep their existing behaviour: the same inventory answers them,
  and their refusal text is unchanged.
- Test and query loads record no expansion or budget telemetry
  ([ADR-0050](adr-0050-bound-manifest-template-evaluation.md) Addendum E),
  since those counters describe normal manifest loading.

## Alternatives considered

- A boolean `test_mode` flag on the load options. Rejected: it splits the
  capability decision across a flag and the existing registration, and admits
  combinations neither mode defines.
- Registering nothing for withheld helpers. Rejected: MiniJinja's unknown-name
  error does not say the capability was withheld, and no typed source is
  available for callers.
- Building a second, test-only stdlib. Rejected: it duplicates the surface,
  drifts from build behaviour, and would need its own diagnostics.
- Intercepting MiniJinja's unknown-function error centrally. Rejected: the
  refusal would lose the operation name and the call-shape reach the stubs
  preserve.
- Granting the real helpers against a sandbox root. Rejected for 7.1.2: `glob`,
  the file tests, and the file-reading filters do not route through
  `StdlibConfig::workspace_root` yet; that work is roadmap 7.4.1.

## Implementation references

- `src/manifest/loader_options.rs` — `StdlibRegistration` and
  `ManifestLoadOptions`.
- `src/manifest/test_surface/mod.rs` — `TestSurface`.
- `src/manifest/test_surface/overlays.rs` — `TemplateOverlays`,
  `OverlayCallable`, `OverlayCall`, name validation, and registration order.
- `src/stdlib/restricted_helpers/mod.rs` — the `SURFACE` inventory.
- `src/stdlib/restricted_helpers/test_mode.rs` — `TestModeRefusal` and the
  refusing registrations.
- `src/stdlib/restricted_helpers/query.rs` — the query disposition and its
  refusal diagnostics.
- `src/manifest/env/reader.rs` — `disabled_env_reader`.

## References

- [Technical design §4.2, §4.3, §5.5, and §7](../netsuke-test-framework-technical-design.md)
- [UX and semantic design §8.6](../netsuke-test-framework-ux-design.md)
- [ADR-0008: environment seam taxonomy](adr-0008-environment-seam-taxonomy.md)
- [ADR-0010: glob capability scoping](adr-0010-scope-the-glob-metadata-capability-to-the-literal-prefix.md)
- [ADR-0050: bound manifest template evaluation](adr-0050-bound-manifest-template-evaluation.md)
- [RFC 0007: Netsukefile testing framework](../rfcs/rfc-0007-netsukefile-testing-framework.md)
- [Roadmap items 7.1.2 and 7.4.1](../roadmap.md#71-seams-and-loader-options)
