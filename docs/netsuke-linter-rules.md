# Netsuke lint rule reference

Every rule `netsuke check` ships, what it detects, why the construct is a
problem, and how to fix it. A contract test checks this document against the
rule registry, so it cannot list a rule that does not exist or omit one that
does.

Rule names are stable. A rule that is retired keeps its name reserved, and a
rule whose meaning changes materially takes a new name, so a name written in a
configuration file or a suppression comment keeps working.

See the [manifest linter design](netsuke-linter-design.md) for the rule model
and the output schemas, and the
[users' guide](users-guide.md#lint-a-manifest-with-netsuke-check) for how to
run the command.

## Configuring a rule

`--rule NAME=SEVERITY` sets the severity of one rule or of a whole category.
`SEVERITY` is `off`, `advice`, `warning`, or `error`. Selectors apply in order,
so a category selector followed by a rule selector narrows it:

```sh
netsuke check --rule clarity=off --rule literal-recipe-path=error
```

Setting any severity on a rule whose default is `off` enables it. The same
selectors layer through a configuration file:

```toml
[cmds.check]
rule = ["clarity=off", "unreachable-target=warning"]
fail_on = "warning"
```

## Suppressing a finding

A suppression names the rules it silences and must state a reason:

```yaml
targets:
  # netsuke-lint: allow background-job -- the viewer must outlive the build
  - name: preview
    phony: true
    script: |
      feh build &
```

A directive on its own line governs the declaration beneath it, together with
everything indented under that declaration. A directive at the end of a line
governs that line's declaration. `# netsuke-lint-file: allow <rule> -- <reason>`
governs the whole manifest, and exists for findings that could not be resolved
to a source span.

Three rules keep suppressions honest: [unknown-suppression](#unknown-suppression),
[suppression-without-reason](#suppression-without-reason), and
[unused-suppression](#unused-suppression).

## Rule catalogue

<!-- markdownlint-disable-next-line MD013 -->
Table: every rule `netsuke check` ships, with its category, stage, and default severity

| Rule | Category | Stage | Default | Detects |
| --- | --- | --- | --- | --- |
| [`undeclared-target-input`](#undeclared-target-input) | correctness | graph | warning | recipe names another target's output without depending on it |
| [`directory-dep-not-order-only`](#directory-dep-not-order-only) | caching | manifest | warning | directory-creating target used as a content dependency |
| [`phony-dep-of-file-target`](#phony-dep-of-file-target) | caching | manifest | warning | file target depends on a phony target through `sources` or `deps` |
| [`bashism`](#bashism) | portability | document | warning | recipe uses a construct `/bin/sh` does not promise |
| [`background-job`](#background-job) | determinism | document | warning | recipe detaches a process with a trailing `&` |
| [`recursive-build-invocation`](#recursive-build-invocation) | determinism | document | warning | recipe invokes a build tool |
| [`builtin-clean-action`](#builtin-clean-action) | redundancy | document | advice | action named `clean` duplicates the built-in `netsuke clean` |
| [`duplicate-rule-recipe`](#duplicate-rule-recipe) | redundancy | manifest | warning | two rules declare identical recipes |
| [`redundant-always`](#redundant-always) | redundancy | document | advice | `always` declared on a target that is already phony |
| [`redundant-dependency`](#redundant-dependency) | redundancy | manifest | advice | path declared under more than one dependency key |
| [`serial-order-without-deps`](#serial-order-without-deps) | redundancy | document | advice | `dependency_order: serial` declared with fewer than two `deps` |
| [`unused-macro`](#unused-macro) | hygiene | document | warning | declared macro that nothing calls |
| [`unused-rule`](#unused-rule) | hygiene | manifest | warning | declared rule that no target or action references |
| [`unused-var`](#unused-var) | hygiene | document | warning | global `vars` entry that no template references |
| [`action-without-description`](#action-without-description) | clarity | document | advice | action declares no `description` |
| [`command-chain-not-list`](#command-chain-not-list) | clarity | document | advice | scalar `command` chains steps with `&&` |
| [`literal-recipe-path`](#literal-recipe-path) | clarity | document | warning | recipe repeats a path the target already declares |
| [`rule-without-description`](#rule-without-description) | clarity | document | off | rule declares no `description` |
| [`unreachable-target`](#unreachable-target) | clarity | graph | off | target reachable from no default and no other target |
| [`legacy-placeholder`](#legacy-placeholder) | migration | document | warning | recipe uses the undocumented `$in` or `$out` placeholder |
| [`manual-ninja-escape`](#manual-ninja-escape) | migration | document | warning | recipe doubles a dollar to escape it for Ninja |
| [`suppression-without-reason`](#suppression-without-reason) | suppression | directive | warning | lint directive states no reason |
| [`unknown-suppression`](#unknown-suppression) | suppression | directive | warning | lint directive names a rule that does not exist |
| [`unused-suppression`](#unused-suppression) | suppression | directive | advice | lint directive suppressed no finding |

## Correctness

The manifest is likely to behave differently from what it says.

### undeclared-target-input

Stage `graph`, default severity `warning`,
diagnostic code `netsuke::lint::undeclared_target_input`.

Recipe names another target's output without depending on it.

Ninja schedules edges concurrently unless a dependency orders them. A recipe
that reads a path another target produces, without declaring that path as a
dependency, races: it succeeds whenever the producer happened to run first, and
fails or reads stale content when it did not. Serial local builds hide this; a
parallel or clean build does not.

Reported:

```yaml
targets:
  - name: app
    command: "cc build/main.o -o {{ outs }}"
  - name: build/main.o
    command: "cc -c src/main.c -o {{ outs }}"
```

Fixed:

```yaml
targets:
  - name: app
    sources: build/main.o
    command: "cc {{ ins }} -o {{ outs }}"
  - name: build/main.o
    command: "cc -c src/main.c -o {{ outs }}"
```

**Remediation:** Declare the path under `sources` if the recipe reads it, or
under `deps` if it only needs it to exist.

## Caching

The declaration defeats change detection or forces needless rebuilds.

### directory-dep-not-order-only

Stage `manifest`, default severity `warning`,
diagnostic code `netsuke::lint::directory_dep_not_order_only`.

Directory-creating target used as a content dependency.

A directory's modification time changes whenever any entry is created or removed
inside it. A target that depends on a directory through `sources` or `deps`
therefore rebuilds whenever a sibling output is written, even though nothing it
reads has changed.

Reported:

```yaml
targets:
  - name: build
    command: "mkdir -p {{ outs }}"
  - name: build/report.txt
    deps: build
    command: "report > {{ outs }}"
```

Fixed:

```yaml
targets:
  - name: build
    command: "mkdir -p {{ outs }}"
  - name: build/report.txt
    order_only_deps: build
    command: "report > {{ outs }}"
```

**Remediation:** Move the directory to `order_only_deps`, which guarantees it
exists first without tracking its timestamp.

### phony-dep-of-file-target

Stage `manifest`, default severity `warning`,
diagnostic code `netsuke::lint::phony_dep_of_file_target`.

File target depends on a phony target through `sources` or `deps`.

A phony target is always considered out of date. A file target that depends on
one through a content key is therefore also always out of date, and so is
everything downstream, which removes incremental rebuilds from that whole branch
of the graph.

Reported:

```yaml
targets:
  - name: schema
    phony: true
    command: "generate-schema"
  - name: api.json
    deps: schema
    command: "render > {{ outs }}"
```

Fixed:

```yaml
targets:
  - name: schema
    phony: true
    command: "generate-schema"
  - name: api.json
    order_only_deps: schema
    command: "render > {{ outs }}"
```

**Remediation:** Move the entry to `order_only_deps`, which sequences the work
without forcing a rebuild.

## Portability

The construct depends on a shell or platform Netsuke does not promise.

### bashism

Stage `document`, default severity `warning`,
diagnostic code `netsuke::lint::bashism`.

Recipe uses a construct `/bin/sh` does not promise.

Netsuke runs `script` recipes under `/bin/sh -e`, and `command` recipes through
the same shell. On a host where `/bin/sh` is `dash` rather than `bash`, a
`bash`-only construct fails at build time with a syntax error that does not
reproduce on the author's machine.

Reported:

```yaml
targets:
  - name: report.txt
    script: |
      if [[ -f input.txt ]]; then
        cat input.txt > report.txt
      fi
```

Fixed:

```yaml
targets:
  - name: report.txt
    script: |
      if [ -f input.txt ]; then
        cat input.txt > report.txt
      fi
```

**Remediation:** Rewrite the construct in POSIX shell, or move the work into a
script the manifest invokes.

## Determinism

The recipe's result depends on something other than its declared inputs.

### background-job

Stage `document`, default severity `warning`,
diagnostic code `netsuke::lint::background_job`.

Recipe detaches a process with a trailing `&`.

A detached process outlives the recipe that started it. Netsuke marks the target
complete as soon as the shell returns, so a later target can consume a
half-written output, and the build can finish while work is still running.

Reported:

```yaml
actions:
  - name: preview
    script: |
      feh build/gallery &
```

Fixed:

```yaml
actions:
  - name: preview
    description: Open the gallery in a viewer
    script: |
      feh build/gallery
```

**Remediation:** Run the command in the foreground, or move the detached work
outside the build into a separate command.

### recursive-build-invocation

Stage `document`, default severity `warning`,
diagnostic code `netsuke::lint::recursive_build_invocation`.

Recipe invokes a build tool.

Netsuke decides the whole graph before Ninja starts. A recipe that invokes
`netsuke`, `make`, or `ninja` hides a second graph inside one edge, so Netsuke
cannot order the two, cannot schedule them against one job budget, and cannot
tell whether the inner build's inputs changed.

Reported:

```yaml
targets:
  - name: vendor/libfoo.a
    command: "make -C vendor libfoo.a"
```

Fixed:

```yaml
targets:
  - name: vendor/libfoo.a
    sources: vendor/foo.c
    command: "cc -c {{ ins }} && ar rcs {{ outs }} foo.o"
```

**Remediation:** Declare the inner build's work as targets in this manifest so
one graph owns all of it.

## Redundancy

The declaration is unnecessary, inert, or duplicated.

### builtin-clean-action

Stage `document`, default severity `advice`,
diagnostic code `netsuke::lint::builtin_clean_action`.

Action named `clean` duplicates the built-in `netsuke clean`.

`netsuke clean` removes exactly the outputs the graph declares, by asking Ninja.
A handwritten `clean` action removes whatever its recipe names, which drifts
from the graph as targets are added and typically reaches for a wildcard that
can delete more than it should.

Reported:

```yaml
actions:
  - name: clean
    command: "rm -f *.o app"
```

Fixed:

```yaml
# Use `netsuke clean`, which asks Ninja for the graph's own outputs.
actions: []
```

**Remediation:** Delete the action and use `netsuke clean`, which is derived
from the graph.

### duplicate-rule-recipe

Stage `manifest`, default severity `warning`,
diagnostic code `netsuke::lint::duplicate_rule_recipe`.

Two rules declare identical recipes.

Two rules with the same recipe are one rule under two names. A change to the
shared command has to be made twice, and a reader cannot tell which name to use
for new targets. Netsuke already deduplicates the generated action, so the
duplication buys nothing at build time.

Reported:

```yaml
rules:
  - name: compile
    command: "cc -c {{ ins }} -o {{ outs }}"
  - name: compile_test
    command: "cc -c {{ ins }} -o {{ outs }}"
```

Fixed:

```yaml
rules:
  - name: compile
    description: Compiling an object file
    command: "cc -c {{ ins }} -o {{ outs }}"
```

**Remediation:** Keep one rule and point the other rule's targets at it.

### redundant-always

Stage `document`, default severity `advice`,
diagnostic code `netsuke::lint::redundant_always`.

`always` declared on a target that is already phony.

A phony target is always considered out of date, and every action is implicitly
phony. Adding `always` to one states the same thing twice and suggests the
author expected it to mean something more.

Reported:

```yaml
actions:
  - name: lint
    always: true
    command: "cargo clippy"
```

Fixed:

```yaml
actions:
  - name: lint
    description: Run the Clippy lints
    command: "cargo clippy"
```

**Remediation:** Remove `always`; the target already runs whenever it is
requested.

### redundant-dependency

Stage `manifest`, default severity `advice`,
diagnostic code `netsuke::lint::redundant_dependency`.

Path declared under more than one dependency key.

`sources`, `deps`, and `order_only_deps` are ordered by strength: a source
rebuilds the target and becomes `{{ ins }}`, an implicit dependency rebuilds it,
and an order-only dependency only sequences it. Declaring one path under two
keys leaves the weaker declaration with no effect, and hides which behaviour the
author wanted.

Reported:

```yaml
targets:
  - name: out.txt
    sources: in.txt
    deps: in.txt
    command: "cp {{ ins }} {{ outs }}"
```

Fixed:

```yaml
targets:
  - name: out.txt
    sources: in.txt
    command: "cp {{ ins }} {{ outs }}"
```

**Remediation:** Keep the strongest declaration the target needs and delete the
other.

### serial-order-without-deps

Stage `document`, default severity `advice`,
diagnostic code `netsuke::lint::serial_order_without_deps`.

`dependency_order: serial` declared with fewer than two `deps`.

Serial ordering sequences the entries of a `deps` list. With no dependencies, or
one, there is nothing to sequence, so the declaration has no effect. It usually
means the dependencies were meant to be listed under `deps` and were written
under `sources` instead.

Reported:

```yaml
actions:
  - name: release
    dependency_order: serial
    command: "./package-release"
```

Fixed:

```yaml
actions:
  - name: release
    dependency_order: serial
    deps:
      - test
      - notes
    command: "./package-release"
```

**Remediation:** List the ordered work under `deps`, or remove
`dependency_order`.

## Hygiene

The declaration is never used.

### unused-macro

Stage `document`, default severity `warning`,
diagnostic code `netsuke::lint::unused_macro`.

Declared macro that nothing calls.

A macro is registered before any other field renders, so an unused one still
costs a reader the effort of understanding it and still occupies the template
namespace that variables and helpers share.

Reported:

```yaml
macros:
  - signature: "greet(name)"
    body: "Hello, {{ name }}!"

targets:
  - name: out.txt
    command: "echo hi > {{ outs }}"
```

Fixed:

```yaml
macros:
  - signature: "greet(name)"
    body: "Hello, {{ name }}!"

targets:
  - name: out.txt
    command: "echo '{{ greet('world') }}' > {{ outs }}"
```

**Remediation:** Delete the macro, or call it from the field it was written for.

### unused-rule

Stage `manifest`, default severity `warning`,
diagnostic code `netsuke::lint::unused_rule`.

Declared rule that no target or action references.

A rule exists to be shared. One that nothing references contributes no build
edge, so it is either dead weight left by a removed target or a rule whose name
a target misspells.

Reported:

```yaml
rules:
  - name: compile
    command: "cc -c {{ ins }} -o {{ outs }}"

targets:
  - name: out.txt
    command: "echo hi > {{ outs }}"
```

Fixed:

```yaml
rules:
  - name: compile
    command: "cc -c {{ ins }} -o {{ outs }}"

targets:
  - name: main.o
    rule: compile
    sources: src/main.c
```

**Remediation:** Delete the rule, or point the target that should share it at
the rule's name.

### unused-var

Stage `document`, default severity `warning`,
diagnostic code `netsuke::lint::unused_var`.

Global `vars` entry that no template references.

An unreferenced variable is usually a rename that was not finished or a recipe
that stopped using it. Either way a reader has to work out whether it still
matters, and a later edit that reintroduces the name silently picks up a stale
value.

Reported:

```yaml
vars:
  cflags: "-Wall"

targets:
  - name: main.o
    command: "cc -c src/main.c -o {{ outs }}"
```

Fixed:

```yaml
vars:
  cflags: "-Wall"

targets:
  - name: main.o
    command: "cc {{ cflags }} -c src/main.c -o {{ outs }}"
```

**Remediation:** Delete the entry, or reference it from the recipe that was
meant to use it.

## Clarity

A canonical alternative reads better or is easier to discover.

### action-without-description

Stage `document`, default severity `advice`,
diagnostic code `netsuke::lint::action_without_description`.

Action declares no `description`.

Actions are a manifest's public entry points, and `netsuke help targets` is how
a newcomer or an agent discovers them. An action without a `description` appears
in that catalogue with no explanation, so the only way to learn what it does is
to read its recipe.

Reported:

```yaml
actions:
  - name: test
    command: "cargo nextest run"
```

Fixed:

```yaml
actions:
  - name: test
    description: Run the unit and behavioural tests
    command: "cargo nextest run"
```

**Remediation:** Add a `description` stating the operation the action performs.

### command-chain-not-list

Stage `document`, default severity `advice`,
diagnostic code `netsuke::lint::command_chain_not_list`.

Scalar `command` chains steps with `&&`.

A `command` list runs its entries in declaration order and stops at the first
non-zero exit, which is what the `&&` chain is emulating. The list form reads as
one step per line and reports which entry failed by position, where the chained
form reports only that the whole command failed.

Reported:

```yaml
rules:
  - name: book
    command: "pandoc {{ ins }} -o book.tex && latexmk -pdf book.tex"
```

Fixed:

```yaml
rules:
  - name: book
    command:
      - "pandoc {{ ins }} -o book.tex"
      - "latexmk -pdf book.tex"
```

**Remediation:** Write the steps as a YAML list under `command`, one entry per
step.

### literal-recipe-path

Stage `document`, default severity `warning`,
diagnostic code `netsuke::lint::literal_recipe_path`.

Recipe repeats a path the target already declares.

Netsuke substitutes and shell-quotes the declared inputs and outputs through `{{
ins }}` and `{{ outs }}`. A recipe that spells the same path out again states it
twice: renaming the target, adding an output, or generating the target with
`foreach` then changes one copy and not the other, and the literal copy is not
shell-quoted.

Reported:

```yaml
targets:
  - name: output.txt
    sources: input.txt
    command: "tr 'a-z' 'A-Z' < input.txt > output.txt"
```

Fixed:

```yaml
targets:
  - name: output.txt
    sources: input.txt
    command: "tr 'a-z' 'A-Z' < {{ ins }} > {{ outs }}"
```

**Remediation:** Replace the literal path with `{{ outs }}` for outputs or `{{
ins }}` for sources.

### rule-without-description

Stage `document`, default severity `off`,
diagnostic code `netsuke::lint::rule_without_description`.

Rule declares no `description`.

Ninja shows a rule's `description` as it runs. Without one it prints the whole
command line, which is noisy for a long compiler invocation and makes a build
log hard to scan.

Reported:

```yaml
rules:
  - name: link
    command: "cc {{ ins }} -o {{ outs }}"
```

Fixed:

```yaml
rules:
  - name: link
    description: Linking an executable
    command: "cc {{ ins }} -o {{ outs }}"
```

**Remediation:** Add a `description` naming the work the rule performs, for
example `Compiling an object file`.

### unreachable-target

Stage `graph`, default severity `off`,
diagnostic code `netsuke::lint::unreachable_target`.

Target reachable from no default and no other target.

A target that no default lists and nothing depends on is only built when someone
names it on the command line. That is a legitimate workflow, so this rule is off
by default; a project that expects every target to be reachable can enable it to
catch the ones left behind by a removed dependency.

Reported:

```yaml
targets:
  - name: app
    command: "cc src/main.c -o {{ outs }}"
  - name: scratch.txt
    command: "echo scratch > {{ outs }}"

defaults:
  - app
```

Fixed:

```yaml
targets:
  - name: app
    command: "cc src/main.c -o {{ outs }}"

defaults:
  - app
```

**Remediation:** Add the target to `defaults`, depend on it from a target that
is reachable, or delete it.

## Migration

A workaround for behaviour that a released version has since changed.

### legacy-placeholder

Stage `document`, default severity `warning`,
diagnostic code `netsuke::lint::legacy_placeholder`.

Recipe uses the undocumented `$in` or `$out` placeholder.

Netsuke substitutes `$in` and `$out` while lowering a recipe, but the users'
guide documents only `{{ ins }}` and `{{ outs }}`. A recipe that meant the shell
variable of the same name is silently rewritten, and a reader cannot tell the
two intentions apart.

Reported:

```yaml
targets:
  - name: out.txt
    sources: in.txt
    command: "cp $in $out"
```

Fixed:

```yaml
targets:
  - name: out.txt
    sources: in.txt
    command: "cp {{ ins }} {{ outs }}"
```

**Remediation:** Write `{{ ins }}` or `{{ outs }}` for Netsuke's paths, and
rename any shell variable that collides.

### manual-ninja-escape

Stage `document`, default severity `warning`,
diagnostic code `netsuke::lint::manual_ninja_escape`.

Recipe doubles a dollar to escape it for Ninja.

Netsuke now escapes dollars at the Ninja writer boundary, after it has lowered
its own placeholders. A recipe that still doubles a dollar reaches the shell as
a literal `$$`, whose first two characters expand to the shell's process
identifier rather than to the intended variable.

Reported:

```yaml
targets:
  - name: out.txt
    command: "printf '%s' \"$$PATH\" > {{ outs }}"
```

Fixed:

```yaml
targets:
  - name: out.txt
    command: "printf '%s' \"$PATH\" > {{ outs }}"
```

**Remediation:** Write the shell variable normally, for example `$PATH` rather
than `$$PATH`.

## Suppression

The lint directives themselves are wrong or stale.

### suppression-without-reason

Stage `directive`, default severity `warning`,
diagnostic code `netsuke::lint::suppression_without_reason`.

Lint directive states no reason.

A suppression without a reason cannot be reviewed. A later reader cannot tell a
considered exception from a silenced defect, so the directive tends to outlive
whatever justified it.

Reported:

```yaml
targets:
  # netsuke-lint: allow background-job
  - name: preview
    phony: true
    script: |
      feh build &
```

Fixed:

```yaml
targets:
  # netsuke-lint: allow background-job -- the viewer must outlive the build
  - name: preview
    phony: true
    script: |
      feh build &
```

**Remediation:** Append `-- <reason>` to the directive, stating why the
construct is correct here.

### unknown-suppression

Stage `directive`, default severity `warning`,
diagnostic code `netsuke::lint::unknown_suppression`.

Lint directive names a rule that does not exist.

A directive naming an unregistered rule silences nothing. It is most often a
typo, or a rule that a Netsuke upgrade renamed or retired, and in both cases the
finding the author meant to suppress is still being reported or is about to
reappear.

Reported:

```yaml
targets:
  # netsuke-lint: allow backgroundjob -- typo in the rule name
  - name: preview
    phony: true
    script: |
      feh build &
```

Fixed:

```yaml
targets:
  # netsuke-lint: allow background-job -- the viewer must outlive the build
  - name: preview
    phony: true
    script: |
      feh build &
```

**Remediation:** Correct the rule name, or delete the directive. `netsuke check
--explain` lists every rule.

### unused-suppression

Stage `directive`, default severity `advice`,
diagnostic code `netsuke::lint::unused_suppression`.

Lint directive suppressed no finding.

A directive that suppresses nothing is usually left over from a problem that has
since been fixed. It then hides the next occurrence of the same problem without
anyone noticing.

Reported:

```yaml
targets:
  # netsuke-lint: allow background-job -- the viewer must outlive the build
  - name: report.txt
    command: "report > {{ outs }}"
```

Fixed:

```yaml
targets:
  - name: report.txt
    command: "report > {{ outs }}"
```

**Remediation:** Delete the directive, or narrow it to the rules it still needs
to silence.
