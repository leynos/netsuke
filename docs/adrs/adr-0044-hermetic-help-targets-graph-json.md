# Architectural decision record (ADR) 0044: Hermetic help-targets graph JSON

## Status

Accepted. `netsuke --json help targets --edges` exports a closed, versioned v1
projection from the restricted manifest-query pipeline. It carries authored
dependency edges and raw legacy recipe bodies without executing recipes.

## Date

2026-10-16

## Context and problem statement

Integrations that audit a Netsukefile need the same dependency declarations,
defaults, conditions, and recipes that Netsuke itself reads. The existing JSON
help-targets catalogue exposes actions, targets, descriptions, defaults, and
conditional flags, but not declaration-level dependency edges or recipes. The
`netsuke graph` command renders DOT or HTML through the full loader and does
not provide a versioned, hermetic JSON contract.

Concordat RFC 0004 needs a stable read-only surface for comparison with its
Makefile reader. A second parser of Netsuke's YAML and Jinja semantics risks
drifting from Netsuke. This export supplies the missing structure while reusing
the existing restricted help-targets query and its validation rules.

## Decision drivers

- Reuse the restricted query manifest already loaded for `help targets`.
- Do not render command or script bodies, execute recipes, or invoke
  host-observing helpers from recipe text.
- Keep the existing help-targets JSON catalogue unchanged unless the caller
  opts into graph data.
- Make the graph payload independently serializable and explicitly versioned
  by the existing JSON envelope.
- Preserve declaration order, authored edge classes, and authored defaults so
  static readers can compare the export with the source manifest.
- Do not confuse a manifest declaration with the expanded names it contains.

## Decision

The opt-in interface is `netsuke --json help targets --edges`. `--json` stays
at the root. Using `--edges` without root `--json` returns a localized runner
diagnostic before manifest loading. The command continues through the existing
Help early return and restricted query loader; it does not resolve Ninja,
select a recipe shell, or construct ambient standard-library configuration for
the export.

The `graph` field is omitted when `--edges` is absent. With the flag present,
the existing action and target catalogue is preserved and the result gains a
separate `graph` object containing:

- `defaults`, copied in authored order and retaining duplicate values;
- `rules`, in declaration order, each with its name, nullable rendered
  description, and recipe;
- `nodes`, with action declarations before target declarations in expansion
  order.

Each node represents one declaration. Its `kind` is `action` or `target`, and
its `names` array carries each expanded name with a `default` Boolean. The node
also carries nullable `description`, `conditional`, `phony`, `always`,
`dependency_order`, `edges`, and `recipe`. The three edge arrays preserve the
authored `sources`, `deps`, and `order_only_deps` values after restricted query
rendering. A node name is marked default exactly when that name is present in
the authored defaults list.

Recipe values use an internally tagged, closed `kind` vocabulary:

- `command` has `form` (`string` or `list`) and a `commands` array containing
  the raw legacy text;
- `script` has raw `script` text;
- `rule` has rendered selector `names`;
- `dependency_only` represents the internal empty-command marker.

The wire model is separate from AST serialization. It has no variables, macros,
shell, argv, or `cwd` fields. Structured-command recipes are deferred until
milestone 12.1.2 defines their AST, and literal `cwd` is deferred until 12.2.3
defines its contract.

The projection is created from the query-mode manifest already in memory. It
does not reload the manifest, render recipes, interpolate placeholders, or call
the command-interpolation layer. Query-mode rendering leaves legacy command,
script, and rule recipe bodies raw. The JSON schema is pinned by a scoped
snapshot that redacts only the generator version.

### Inherited validation limits

The export intentionally retains the existing help-targets validation order and
scope. Terminal-control checks run on catalogue names, followed by catalogue
collection, `BuildGraph` construction from the filtered manifest, and
default-reference validation. It does not weaken or split
`BuildGraph::from_manifest` validation.

As a result:

- an unresolved conditional declaration stays visible with
  `conditional: true`, but is excluded from graph validation;
- a lowering or graph-construction error on a validated declaration fails the
  export and produces no partial JSON result;
- a query-disabled host-dependent helper used in `foreach` data, or in a name,
  edge, description, or rule selector, fails the query;
- a query-disabled helper in a `when` condition retains the declaration as a
  conditional alternative, following the existing catalogue contract.

The restricted query does not evaluate helpers in command and script bodies.
For shell filters in query-rendered metadata, an omitted `dialect` uses a fixed
`sh` dialect on every host, while an explicit dialect is honoured as written.
This keeps the metadata projection independent of the build shell and avoids
resolving shell configuration during a read-only query. When recipe bodies are
later rendered for a build, those helpers use the recipe shell's dialect, or
the host default when no shell is selected. Authors who need stable rendered
command text should pin the helper dialect. The graph export preserves recipe
text as authored and does not claim that future recipe rendering is
host-independent.

### Configuration-default disposition

The host-default dialect in `StdlibConfig` remains the default for full build
rendering. The runner resolves its selected `RecipeShell` once and passes it
through the full manifest-load inputs; full registration stores that shell's
dialect with `StdlibConfig::with_recipe_shell`. The restricted query loader
selects `StdlibRegistration::ManifestQuery` directly; it does not construct
`StdlibConfig`, and query helper registration pins `shell_quote` and
`shell_join` to `ShellDialect::Sh`. The help early return also happens before
the runner resolves a recipe shell. Changing the configuration default would
therefore alter fallback behaviour for full rendering without affecting query
metadata, so the query-specific registration is the correct boundary for
host-independent metadata.

The manifest-facing `ShellSelection` vocabulary added on `main` belongs to the
future structured-command contract
([ADR-0019](adr-0019-select-allow-listed-structured-command-shells.md),
milestone 11.2.1). It is deliberately separate from legacy `RecipeShell`
selection and MiniJinja helpers. Graph v1 exports raw legacy command text and
has no structured command or selected-shell field; it does not infer that
vocabulary from a legacy recipe. When structured commands are added under
milestone 12.1.2, their export should use the established selection vocabulary
without changing the query metadata dialect or `StdlibConfig` default.

## Alternatives considered

### Extend `netsuke graph` with JSON

This would place the export on the full loader path and conflate a rendered
visualization with a static manifest-query contract. It is rejected because the
graph command is not hermetic and the requirement is to avoid recipe and
host-helper evaluation.

### Add edge fields to existing catalogue rows

This would change the established `help-targets` JSON shape for every caller
and would repeat one declaration's edges for each expanded name. It is rejected
in favour of an opt-in, declaration-level graph projection.

### Export serialized AST values

AST serialization follows the manifest input representation and is not the
public graph contract. It could also expose implementation details as the
schema. A separate wire model is selected so recipe variants, nullable fields,
and ordering are deliberate.

## Consequences

The graph JSON provides an in-process source of truth for declaration-level
structure without introducing a second parser or changing the existing
catalogue. Its output is deterministic for the same manifest and query
environment and is useful for audits that do not require build execution.

The export does not predict fully rendered command text, per-command working
directories, or structured argv. Consumers must treat recipes as source text
and respect `conditional` entries. Query validation can still reject a
manifest, and this export inherits those checks rather than serving as a
syntax-only reader.
