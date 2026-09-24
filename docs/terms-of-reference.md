# Netsuke — terms of reference

- **Status:** Draft (v0.1). Reconstructed from existing artefacts; several
  items await confirmation by the maintainer (see
  [section 9](#9-open-questions)).
- **Audience:** The maintainer, contributors and reviewers deciding what
  Netsuke should and should not become, and anyone writing a design document,
  Request for Comments (RFC), or roadmap phase that needs a defensible problem
  statement to trace back to.
- **Companion documents:** `docs/netsuke-design.md` (architecture),
  `docs/netsuke-cli-design-document.md` (command-line interface),
  `docs/roadmap.md` and `docs/roadmap-composition.md` (sequencing),
  `docs/adr-003-agent-consistent-human-first-cli.md`,
  `docs/adr-021-trust-aware-fetch-policy-merge.md`,
  `docs/adr-026-manifest-environment-access-policy.md`, and the RFCs under
  `docs/rfcs/`. There is no `docs/context.md` yet; see
  [appendix B](#appendix-b-glossary).
- **Last revised:** 2026-09-24.

## Reading this document

This document states the problem Netsuke addresses, for whom, and within what
bounds. It does not describe how Netsuke works; `docs/netsuke-design.md` does
that.

It was written after the design document, the roadmap, and nearly forty
architectural decision records (ADRs). The usual order is the reverse. The
document therefore records the premises the existing design already assumes, so
that later design work can test itself against them rather than against each
author's recollection.

Each substantive claim has one of three statuses:

- **Established** claims cite a source in the repository or in the recorded
  design conversations and carry no marker.
- **Assumed** claims are marked with an assumption identifier, such as
  `(A3)`, which resolves to [section 8.2](#82-assumptions) with its failure
  consequence.
- **Open** matters are marked with a question identifier, such as `(Q2)`,
  which resolves to [section 9](#9-open-questions).

## 1. Background and motivation

GNU Make encodes a small, domain-agnostic idea: this file needs building, it
needs those files, and here is how to build it. That idea has lasted almost
fifty years because it is indifferent to what is being built. The notation
around it has not aged as well: tab-sensitive syntax, automatic variables such
as `$@` and `$^`, implicit suffix rules, recursive invocation hazards, and
quoting that is delegated wholesale to the shell.

The alternatives that grew up since then split into two camps, with little
between them:

- **Task runners** (`just`, Task, Mage, Invoke) fix the notation but mostly
  give up the dependency graph, or keep only a light per-task freshness check.
- **Hermetic build systems** (Bazel, Buck2, Pants) keep a rigorous graph and
  add sandboxing, remote execution, and remote caching, at the price of a new
  language (Starlark), a new filesystem model, and a substantial adoption
  effort.

Ninja, meanwhile, provides a fast, static, domain-agnostic executor, and says
plainly that it expects a generator above it. The generators that exist (CMake,
Meson, GN, xmake) are organized around compiling C and C++ and carry opinions
about toolchains, installation, and project layout.

Three conditions make a general-purpose generator worth building now:

1. **The accidental-build-system problem has grown.** A typical repository
   now carries a Makefile or `justfile`, several shell scripts, continuous
   integration (CI) YAML that restates the same commands, and pinned tool
   invocations for linters and formatters across several languages. The
   Makefile in `leynos/cuprum` at revision `08c7665` is the reference specimen:
   most of it is file selection, tool-environment construction, and worker-flag
   coordination rather than a description of the project.
2. **Automated agents now author and run build logic.** A build description
   that resolves to a static, inspectable plan before any command runs is far
   easier for an agent to author, check, and explain than an imperative script.
   ADR-003 made agent-consistent structure a product requirement `(A7)`.
3. **The implementation substrate is mature.** A Jinja implementation in Rust
   (`minijinja`), a YAML 1.2 parser with source-aware diagnostics
   (`serde-saphyr`), and a stable Ninja make a compiler of this shape tractable
   for a small team.

The motivation is also, candidly, personal. The project began when the
maintainer went looking for "Make, without the painful syntax and the
dependence on shell" and found that the tool did not exist: an assistant asked
for recommendations invented two of the three it offered. Netsuke's first user
is its maintainer's own estate of repositories `(A1)`. That is a legitimate
origin, but it means the external demand described in sections 3 and 4 is
inferred from the landscape rather than measured `(Q2)`.

## 2. Domain

### 2.1 Field of practice

Netsuke operates in build automation: turning source files and declared
intentions into derived artefacts and completed side effects, doing only the
work whose inputs have changed, and running independent work in parallel.

### 2.2 Established conventions

Practitioners in this field share several conventions that Netsuke inherits
rather than questions:

- **The dependency graph is the model.** Work is a directed acyclic graph of
  edges from inputs to outputs; cycles are errors.
- **Modification time decides freshness for local files.** Make and Ninja
  rebuild an output when an input is newer. It is cheap, universally
  understood, and good enough for local sources; its weaknesses (clock skew,
  preserved timestamps) are well known and tolerated `(A5)`.
- **Phony and always-run targets exist.** Some nodes name side effects rather
  than files (`clean`, `test`, `lint`), and some must run on every invocation.
- **Recipes are commands.** The unit of work is an external program, usually
  launched through a shell. The build tool does not know what the program does.
- **Generators and executors are separate layers.** Ninja's authors describe it
  as an assembler for build systems; CMake, Meson, and GN are its compilers.
- **Hermeticity is a spectrum, not a switch.** At one end, Make trusts the
  ambient environment completely; at the other, Bazel and Nix control every
  input. Most projects sit in between and want to know *where* they sit.

### 2.3 Trust and purity

Two domain facts shape what a build tool can promise:

- **A build description executes code.** A `Netsukefile`, like a `Makefile`,
  runs arbitrary commands. Netsuke can reduce quoting mistakes; it is not a
  sandbox, and the README says so.
- **The checkout is less trusted than the operator.** A project's manifest is
  authored by whoever controls the repository, while credentials, tokens, and
  network access belong to the person or CI job running the build. ADR-021 and
  ADR-026 established that project configuration must not grant itself
  authority the operator has not granted.

Anything that consults the network, the clock, the environment, or a subprocess
while the plan is being produced makes that plan depend on something outside
the repository. The domain calls such a plan *impure*. Netsuke already tracks
impurity for its template helpers; how far that concept extends to targets and
remote inputs is open `(Q4)`.

### 2.4 Prior art

| Tool           | Relationship to Netsuke                                                                                        |
| -------------- | -------------------------------------------------------------------------------------------------------------- |
| GNU Make       | The semantic ancestor. Netsuke keeps its graph model and discards its notation.                                |
| Ninja          | The executor Netsuke targets. Netsuke exists because Ninja is unpleasant to write by hand.                     |
| Shake, redo    | Demonstrations of rigorous, domain-agnostic build semantics; neither reached a broad audience.                 |
| Bazel, Nix     | The reference for ownership of outputs, typed configuration, toolchains, and content identity.                 |
| `just`         | The reference for ergonomics: visible parameters, readable recipes, and a pleasant first five minutes.         |
| Ansible        | The source of the `foreach`/`when` idiom for declarative repetition and conditions in YAML.                    |
| GitHub Actions | Evidence that templated YAML is an accepted authoring format among the target users, whatever its critics say. |

## 3. Market context

### 3.1 Alternatives users already have

| Alternative                        | What it does well                                                            | Where it falls short for Netsuke's users                                                               |
| ---------------------------------- | ---------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| GNU Make                           | Universal, domain-agnostic, installed everywhere                             | Hostile notation; shell quoting is the author's problem; large Makefiles become unreviewable           |
| Shell scripts, `build.py`, CI YAML | No new tool to learn; the current default for many projects                  | No graph, no incrementality, no parallelism; logic is duplicated between local scripts and CI          |
| `just`                             | Excellent ergonomics and diagnostics; parameterized recipes                  | A command runner with no freshness model                                                               |
| Task (Taskfile)                    | YAML recipes, includes, watch mode, checksum or timestamp freshness per task | Freshness is per task rather than a compiled graph; templating can reshape the file at run time        |
| Mage, Rake, Invoke                 | The full power of a general-purpose language                                 | Imperative build logic that becomes hard to inspect or parallelize                                     |
| CMake, Meson, GN, xmake            | Mature Ninja generators with toolchain and install support                   | Organized around compiling C and C++; opinions about layout and toolchains                             |
| Bazel, Buck2, Pants, Please        | Hermetic, reproducible, remotely executable monorepo builds                  | New language, new filesystem model, and an adoption cost that small and medium projects cannot justify |
| Writing `build.ninja` by hand      | Maximum speed and control                                                    | No variables, conditions, or globbing; unmaintainable beyond a toy                                     |

### 3.2 The gap

No widely used tool combines all four of these properties:

1. a general-purpose, domain-agnostic dependency graph with Make's semantics;
2. an authoring format that ordinary developers can read and write without
   learning a build-specific language;
3. a plan that is fully resolved, validated, and inspectable before any
   command runs, and identical for identical inputs; and
4. an adoption cost measured in minutes rather than weeks.

Netsuke aims at that "static middle": more than a task runner, far less than
Bazel. The risk of the middle is that it is squeezed from both sides. A tool
that chases developer-workflow conveniences will be compared with Task and
lose; a tool that chases sandboxing and remote caching will be compared with
Bazel and lose. Section 6.2 exists largely to hold that line.

## 4. Users and stakeholders

### 4.1 Primary users

The design conversations identify three primary user groups. They form a
progression in how much they know about build systems, not three separate
products.

Netsuke initially serves the reluctant Make users: developers who struggle to
let go of Make because task runners are too limited and Bazel and CMake are too
complex to justify. The maintainer settled this on 2026-09-24 (Q1, resolved).
The other two groups are later audiences. The design should not close doors to
them, but where their needs conflict with the initial group's, the initial
group takes precedence `(A8)`.

| User group                              | Context                                                                                                       | Cares about                                                                           | Ignores or dislikes                                                                                | Current alternative                  |
| --------------------------------------- | ------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- | ------------------------------------ |
| Reluctant Make users (initial)          | Fluent Make users who value its small core and inspectability, and have tried the alternatives                | A domain-agnostic graph with freshness checks, transparency, no hidden magic, speed   | Task runners' missing graph; Bazel's and CMake's ceremony; anything that feels like a new religion | GNU Make                             |
| Bazel-curious, Bazel-repelled (later)   | Engineers who understand why hermeticity, reproducibility, and dependency graphs matter                       | Correctness, determinism, reviewable build changes, explicit inputs                   | Starlark, toolchain ceremony, a new filesystem model, "attaining enlightenment" first              | Make or scripts, with private guilt  |
| Accidental build-system authors (later) | Developers with scripts, a `justfile`, CI YAML, and a load-bearing `build.py`, who deny having a build system | Commands that run, incremental speed without having to think about it, readable files | Build-system theory, long documentation, anything longer than their current script                 | Shell scripts, `just`, Task, CI YAML |

The reluctant Make users recognize exactly what Netsuke fixes, because they
live with the problem daily; they are the likeliest early adopters and
contributors. The Bazel-curious share their values and follow naturally. The
accidental build-system authors are probably the largest group in the long run
`(A2)`, but they are not the group Netsuke is first built for.

### 4.2 Secondary users

| User                            | Interaction                                                                                          |
| ------------------------------- | ---------------------------------------------------------------------------------------------------- |
| Automated agents and CI systems | Author, validate, and run manifests; consume `--json` output. ADR-003 makes them a design audience.  |
| Reviewers                       | Read manifest changes and generated plans in pull requests, without necessarily running them.        |
| Operators of a checkout         | Run someone else's manifest and decide what network, environment, and filesystem authority to grant. |
| Netsukefile test authors        | Write manifest-time tests (`docs/rfcs/0007-netsukefile-testing-framework.md`).                       |

### 4.3 Stakeholders

| Stakeholder                       | Interest                                                                                                  |
| --------------------------------- | --------------------------------------------------------------------------------------------------------- |
| The maintainer (df12 Productions) | Owns direction; uses Netsuke across their own repositories; funds the work in time.                       |
| The df12 repository estate        | The first real workload. Cuprum and sibling repositories supply migration benchmarks.                     |
| OrthoConfig maintainers           | Netsuke depends on OrthoConfig for command, configuration, and schema machinery (`docs/roadmap.md`).      |
| Downstream packagers              | Debian, RPM, macOS, and Windows installer consumers who need predictable releases and a Ninja dependency. |

### 4.4 Non-users

| Non-user                                                                               | Better served by                                      |
| -------------------------------------------------------------------------------------- | ----------------------------------------------------- |
| Organizations needing sandboxed, remotely executed builds with a shared remote cache   | Bazel, Buck2, Pants                                   |
| C and C++ projects needing toolchain detection, install rules, and IDE project export  | Meson or CMake                                        |
| Teams wanting a live development loop: file watching, dev servers, interactive prompts | Task, or `inotifywait` or `watchexec` running Netsuke |
| Projects whose whole build is one language's native tool and needs nothing around it   | Cargo, `uv`, npm, Gradle                              |

## 5. Jobs to be done

### 5.1 Reluctant Make users (initial primary users)

> When a Makefile has become hard to read, quote, or change safely, and the
> alternatives they have tried are either too limited (task runners with no
> dependency graph) or too demanding (Bazel, CMake), a seasoned Make user wants
> to keep Make's graph semantics while replacing its notation and
> shell-quoting hazards, so they can finally leave Make without giving up
> anything it did for them.

- **Functional:** targets, dependencies, phony and always-run nodes, and
  order-only dependencies, with no loss of generality.
- **Emotional:** the sense that nothing was taken away.
- **Social:** no need to defend a trendy tool to other Make users.

### 5.2 Bazel-curious, Bazel-repelled (later)

> When their project has outgrown scripts and they need builds they can trust
> and review, a correctness-minded engineer wants to describe the build as an
> explicit, checkable graph without adopting a new build language and
> filesystem model, so they can get reproducibility and reviewable build
> changes at a cost their team will accept.

- **Functional:** a plan that is identical for identical inputs and can be
  inspected, diffed, and explained before it runs.
- **Emotional:** confidence that nothing varies without their knowledge.
- **Social:** being the colleague who improved the build rather than the one
  who imposed Bazel.

### 5.3 Accidental build-system authors (later)

> When a repository's scripts, CI steps, and task-runner recipes have drifted
> apart and slow down every change, a developer who "does not need a build
> system" wants one readable file that runs the right commands in the right
> order and skips work that is already done, so they can stop maintaining
> glue and trust that local runs match CI.

- **Functional:** one command for the common workflow; the same file drives
  local runs and CI.
- **Emotional:** the pain stops without a detour through build-system theory.
- **Social:** a repository newcomers can build on their first day.

### 5.4 Agents and CI (secondary)

> When an automated agent must change or run a project's build, it wants a
> build description whose effects are resolved, validated, and reported in a
> structured form before execution, so it can act without guessing and
> explain what it did.

## 6. Scope

### 6.1 Goals

Each goal is phrased so that an observer can check it.

1. **G1 — A shallow end.** A newcomer can write and run a working manifest
   using only targets, commands, and defaults, after reading one page, with no
   concept introduced before it solves a problem the user has. The existing
   quick start is the regression fixture for this goal.
2. **G2 — Progressive enhancement, never contagion.** Every advanced semantic
   (structured commands, typed inputs, managed states, artefact ownership,
   contention classes, strictness policies) is opt-in, local to the entry that
   uses it, and never makes a simpler manifest longer or invalid. A project can
   stop on any rung: command runner, typed task graph, or reproducible
   orchestration.
3. **G3 — Domain agnosticism.** Netsuke builds anything a command can build and
   ships no blessed language or toolchain in its core.
4. **G4 — Deterministic plans.** Given the same manifest, environment, and
   filesystem state, Netsuke produces a byte-identical Ninja plan.
5. **G5 — Inspect before execute.** Users and agents can list, render, graph,
   and validate the full plan, including which entries are conditional, without
   running a recipe.
6. **G6 — Visible impurity.** Every input the plan draws from outside the
   repository (network, environment, clock, subprocess) is declared or
   detectable, and the operator, not the checkout, grants that authority.
7. **G7 — Safe command construction by default.** Paths Netsuke substitutes
   reach programs as intended arguments, and an argv-based command form exists
   that involves no shell at all `(A6)`.
8. **G8 — Actionable failures.** Every diagnostic states what failed, where in
   the manifest, and what to do next, in human-readable and versioned
   machine-readable forms.
9. **G9 — Parity across Linux, macOS, and Windows** for the documented manifest
   model, with any platform-specific behaviour stated explicitly.
10. **G10 — Replace real monster Makefiles.** A migrated manifest for a
    repository of Cuprum's complexity consists mainly of lines that describe
    that project, not lines that compensate for the build language.

### 6.2 Non-goals

1. **A new scheduler or executor.** Ninja schedules and executes; Netsuke
   compiles. Resource and ordering needs lower to Ninja's facilities rather
   than to a second scheduler.
2. **Hermetic sandboxing, remote execution, or a shared remote cache.** Users
   who need these should use Bazel, Buck2, or Pants. Netsuke makes impurity
   visible; it does not eliminate it.
3. **Package or toolchain management.** Netsuke does not resolve or install
   dependencies. It may describe and verify a tool environment that `uv`,
   Cargo, or a system package manager provides.
4. **Out-guessing native incremental builds.** Netsuke does not reimplement
   Cargo's or any other tool's incremental graph; it delegates.
5. **A general-purpose programming language in the manifest.** Jinja renders
   values; structure changes only through `foreach` and `when`. Users who want
   imperative build code should use Mage, Invoke, or a script that Netsuke
   calls.
6. **A live development loop.** File watching, long-running dev servers, and
   interactive prompts are out of scope, confirmed by the maintainer on
   2026-09-24. Users who want rebuild-on-save should drive Netsuke from
   `inotifywait`, `watchexec`, or a task runner, as Make users already do with
   Make.
7. **A security sandbox for untrusted manifests.** Reviewing a manifest before
   running it remains the operator's responsibility, as with a Makefile.
8. **Inferring semantics from command text.** Netsuke will not decide that
   `uv sync` creates an environment or that `rm -rf build` owns `build`.
   Semantics are declared, not guessed.
9. **Mandatory annotation.** No strictness rule applies unless a project opts
   into it.
10. **C and C++ project conveniences as core features.** Toolchain detection,
    install rules, and IDE export belong to Meson and CMake, or to optional
    rule bundles.

## 7. Success criteria

### 7.1 User-facing

- The quick-start manifest in `docs/quickstart.md` works unchanged in every
  release, and a newcomer completes the guide in under five minutes `(A4)`.
- The Cuprum Makefile at `08c7665` migrates to a Netsukefile that keeps its
  deliberate safeguards (restricted extension-test selection, interpreter
  requirements, extension preconditions) and removes repository-authored file
  transport, duplicated tool pins, and hand-coordinated worker flags.
- The maintainer's own repositories use Netsuke as their primary build entry
  point. The target count and date are open `(Q2)`.
- External adoption: a signal and threshold are not yet defined `(Q2)`.

### 7.2 Operational

- Snapshot tests show byte-identical Ninja output for identical inputs on
  Linux, macOS, and Windows.
- Every `--json` document validates against its published, versioned schema.
- A manifest that consults the network or environment without an operator
  grant fails before any recipe runs.
- Plan generation time stays small relative to the Ninja run it precedes. No
  budget is set yet `(Q7)`.

### 7.3 Strategic

- Netsuke reaches 1.0 with a stable manifest schema and command-line contract.
  The release criteria are open `(Q8)`.
- The progressive-enhancement features proposed in PR #741 are delivered
  without changing the shallow-end fixture.
- A reference repository demonstrates Netsuke in a real multi-language
  workflow, including its generated plan under review in a pull request.

## 8. Constraints and assumptions

### 8.1 Hard constraints

- **Ninja is the execution backend.** Any capability must be expressible as a
  static Ninja graph, possibly with `dyndep`, produced before execution begins.
- **The authoring format is YAML with Jinja.** This decision predates this
  document and is embodied in every manifest written so far `(A3)`.
- **Manifests execute arbitrary commands.** No design may claim sandboxing it
  does not provide.
- **Project configuration cannot grant itself operator authority**
  (ADR-021, ADR-026).
- **Licensing and openness.** Netsuke is ISC-licensed and developed in public.
- **Pre-1.0 status.** Interfaces may change before 1.0; after 1.0, the manifest
  schema version (`netsuke_version`) governs compatibility.
- **Source builds need the pinned nightly Rust toolchain** (ADR-006). Prebuilt
  binaries and installers avoid this for users, but not for contributors or
  registry installs.

### 8.2 Assumptions

| ID  | Assumption                                                                                                                              | Consequence if false                                                                                                                |
| --- | --------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| A1  | The maintainer's own repositories are representative of the wider target users.                                                         | Features tuned to the df12 estate (for example Python-and-Rust quality gates) misfire for other users; external research is needed. |
| A2  | Accidental build-system authors are the largest group, and they will adopt a graph-based tool if its first five minutes match `just`'s. | Adoption stalls outside expert users; onboarding and defaults need rework.                                                          |
| A3  | Target users accept YAML with Jinja as an authoring format.                                                                             | Reluctant Make users reject the tool on sight; an alternative surface or stronger justification is needed.                          |
| A4  | Users can install Ninja, or accept an installer that depends on it.                                                                     | Onboarding fails at the first step on platforms without packaged Ninja; bundling or fetching Ninja becomes necessary.               |
| A5  | Modification-time freshness is sufficient for local files.                                                                              | Users hit spurious or missed rebuilds; content-hash invalidation moves into scope `(Q5)`.                                           |
| A6  | Shell-string recipes remain acceptable while structured commands mature.                                                                | Quoting failures on Windows or with unusual filenames erode trust before the safer form ships.                                      |
| A7  | Agents benefit materially from a static, structured plan compared with an imperative script.                                            | ADR-003's investment in agent-consistent output delivers less value than expected.                                                  |
| A8  | Serving reluctant Make users first does not preclude serving the other two groups later.                                                | A feature the later groups need is blocked by an early decision; migrating them requires breaking changes.                          |

### 8.3 Dependencies

| Dependency                                                   | Role                                             | Critical path                               |
| ------------------------------------------------------------ | ------------------------------------------------ | ------------------------------------------- |
| Ninja                                                        | Executes every build                             | Yes: every user needs it at run time        |
| OrthoConfig                                                  | Command, configuration, and schema machinery     | Yes for command-line and configuration work |
| `minijinja`, `serde-saphyr`                                  | Template evaluation and YAML parsing             | Yes for manifest semantics                  |
| Pinned nightly Rust (Polonius, next-generation trait solver) | Compiles Netsuke                                 | Yes for contributors and source installs    |
| The df12 repository estate                                   | Supplies migration benchmarks and first real use | Yes for G10 and the user-facing criteria    |

## 9. Open questions

| ID  | Question                                                                                                                                                                                                                                                 | Why it matters                                                                                                      | Resolved when                                                                                                  | Suggested path            |
| --- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- | ------------------------- |
| Q2  | What adoption signal defines success, inside the df12 estate and outside it?                                                                                                                                                                             | Without it, sections 1, 4, and 7 rest on inference.                                                                 | A named signal with a threshold and date, for example repositories using Netsuke as their primary entry point. | Elicitation               |
| Q3  | Reluctant Make users reject task runners as too limited, yet Make also serves them as a task runner. Which task-runner conveniences (recipe parameters, target listing, per-task help) are in scope? Non-goal 6 (no live development loop) is confirmed. | Sets the boundary with `just` and Task, and decides whether Netsuke replaces every job the initial users give Make. | The maintainer lists the task-runner features that are in or out.                                              | Elicitation, then ADR     |
| Q4  | How far does purity extend: to targets that call impure helpers, and to pinned remote inputs used as dependencies? What rebuild policy applies to an impure target?                                                                                      | Determines whether remote inputs become first-class and how G4 and G6 interact.                                     | A decision on impure-target rebuild semantics and on remote resources as graph inputs.                         | RFC                       |
| Q5  | Is content-hash invalidation in scope for any input class?                                                                                                                                                                                               | Tests assumption A5; affects remote inputs and any future cache.                                                    | Evidence of missed or spurious rebuilds in real use, or a decision tied to Q4.                                 | Spike, then ADR           |
| Q6  | Is a Makefile migration aid (importer or guide) in scope?                                                                                                                                                                                                | Directly serves the initial reluctant-Make-user group; costs significant effort.                                    | A decision recorded as a goal or a non-goal.                                                                   | Elicitation               |
| Q7  | What plan-generation time budget is acceptable, and on what reference manifest?                                                                                                                                                                          | Needed to turn the operational criterion into a measurable one.                                                     | A budget and a benchmark manifest exist.                                                                       | Spike                     |
| Q8  | What must be true for 1.0?                                                                                                                                                                                                                               | Gates the strategic criterion and the end of pre-1.0 latitude.                                                      | A written release checklist traced to G1 to G10.                                                               | Elicitation, then roadmap |
| Q9  | Who maintains reusable rule bundles for common ecosystems, and are any shipped with Netsuke?                                                                                                                                                             | Decides whether G3 and G10 are met by the core or by an ecosystem that does not yet exist.                          | An ownership and distribution decision for bundles.                                                            | RFC amendment to RFC 0003 |
| Q10 | Does the nightly toolchain requirement for source installs conflict with serving the accidental build-system authors (a later audience)?                                                                                                                 | Affects installation paths and constraint 8.1.                                                                      | Evidence that binary installers cover the target platforms, or a plan for stable builds.                       | Elicitation               |

### 9.1 Resolved questions

- **Q1 — Which primary user group does Netsuke 1.0 serve first?** Resolved
  2026-09-24 by the maintainer: reluctant Make users, who struggle to let go of
  Make because task runners are too limited and Bazel and CMake are too
  complex. See [section 4.1](#41-primary-users).
- **Q3, in part — Does non-goal 6 (no live development loop) stand?** Resolved
  2026-09-24 by the maintainer: yes. File watching belongs to `inotifywait` and
  similar tools, which can run Netsuke. The rest of Q3 remains open.

## 10. Handoff

- **Downstream readiness.** The design document and roadmap already exist, so
  this document is a reconciliation rather than a precursor. Q3 should be
  settled before further user-facing design, because it changes what the
  shallow end must contain.
- **ADR candidates.** Primary user segment (Q1, decided 2026-09-24; the ADR
  records it); relationship to task runners (Q3); purity semantics for targets
  and remote inputs (Q4); content-hash invalidation (Q5); hermeticity and
  remote execution as a permanent non-goal (non-goal 2).
- **Glossary.** `docs/context.md` does not exist. The terms in
  [appendix B](#appendix-b-glossary) are the proposed first entries.
- **Design-document candidates.** The following design ideas arose in the
  source conversations and belong in RFCs rather than here: first-class file
  sets, reusable tool contexts, managed states with functional probes, typed
  task inputs, artefact ownership and scoped cleanup, named contention classes,
  maturity policies, and plan explanation and diffing commands.

## Appendix A. References

- `docs/netsuke-design.md`, sections 1 and 9.3.
- `docs/roadmap.md`, "How to read this roadmap" and "Canonical public
  vocabulary".
- `docs/quickstart.md`.
- `README.md`, "Security and command interpolation" and "Release and
  development status".
- ADR-003, ADR-006, ADR-021, and ADR-026 under `docs/`.
- Pull request #741, proposing RFCs 0021 to 0025 on managed states, typed task
  inputs, artefact ownership, contention classes, and progressive enhancement.
- The `leynos/cuprum` Makefile at revision
  `08c766550504fa39009d6503e4fdf3de77c14bd3`.
- Recorded design conversations between the maintainer and an assistant on
  target users, Taskfile comparison, remote dependencies, and the Cuprum
  migration (not published).
- Ninja manual: <https://ninja-build.org/manual.html> (accessed 2026-09-24).

## Appendix B. Glossary

Proposed first entries for `docs/context.md`:

- **Netsukefile:** the YAML manifest describing a project's build.
- **Rule:** a named, reusable recipe template.
- **Target:** a graph node naming one or more output files.
- **Action:** a graph node naming a side effect rather than a file; phony by
  default.
- **Recipe:** how a node runs: a rule reference, a command, or a script.
- **Aggregate:** a node with dependencies and no recipe.
- **Plan:** the static, validated build graph Netsuke produces, and the Ninja
  file generated from it.
- **Impure:** describes a plan or helper that consults the network, clock,
  environment, or a subprocess.
- **Operator:** the person or CI job running a manifest, who owns credentials
  and grants authority.
- **Shallow end:** the minimal subset of the manifest model that a newcomer
  needs: targets, commands, and defaults.
- **Progressive enhancement:** adding stronger semantics to one entry without
  requiring them anywhere else.
