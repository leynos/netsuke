# RFC 0036: Shell parsing and verified transformations

## Preamble

- **RFC number:** 0036
- **Status:** Proposed
- **Created:** 2026-11-01
- **Audience:** Compiler, backend, security, and verification maintainers.
- **Scope:** Shell parsing, validation, bounded model checking, and formal
  proofs for recipe transformations and their execution boundaries.
- **Delivery:** A mandatory programme of follow-up implementation issues and
  architecture decision records (ADRs), coordinated by a tracking meta-issue
  that maintainers MUST raise after this RFC merges.
- **Relationship to immediate fixes:** Follow-up to the five defect families
  in table 1. Their originating issues retain their own remediation and
  acceptance criteria.

## 1. Summary

Delegate supported shell grammar to an established parser and verify the
smaller transformations Netsuke owns: insertion admission, contextual encoding,
source replacement, path preservation, interpreter binding, and transport
composition. Qualify `brush-parser` first for POSIX and Bash syntax. Use
`mvdan/sh` and actual supported interpreters as independent references, and use
Microsoft's version-matched PowerShell parser as the Windows syntax oracle.

The named issue fixes address the immediate defects. They MUST proceed without
waiting for this RFC, parser adoption, or the proof programme. This proposal
does not replace those fixes, close their issues, or add the complete programme
to the v0.1.0 release gate.

Ad hoc fixes MUST subsequently be harmonized under the shared contracts and
verification process in this RFC. Closing an originating bug does not discharge
that obligation. After merge, maintainers MUST raise a tracking meta-issue and
the follow-up issues in section 8, record the relevant ADR decisions, and
retain traceability from each immediate fix to its final implementation and
evidence.

The uppercase terms MUST, MUST NOT, and SHOULD distinguish required programme
obligations, prohibitions, and recommendations. The proposed requirements take
effect through RFC acceptance and the implementation decisions described here;
the document does not claim that the proposed machinery already exists.

## 2. Problem and existing contracts

### 2.1. Different failures require different boundaries

The issue reports describe several failure classes. Their titles and mechanisms
define the initial regression inventory; this RFC does not claim a new runtime
reproduction or fix any reported defect.

| Origin                                                 | Reported failure                                                         | Contract the follow-up must establish                                                                                 |
| ------------------------------------------------------ | ------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------- |
| [#998](https://github.com/leynos/netsuke/issues/998)   | Unicode whitespace shortens a heredoc delimiter in Netsuke's model.      | Classify delimiter and body boundaries under the selected shell grammar; never insert a binding in a protected body.  |
| [#984](https://github.com/leynos/netsuke/issues/984)   | An active template backslash cancels protection added to a replacement.  | Reject unsafe insertion boundaries, or encode the complete supported context through one admission and encoding path. |
| [#990](https://github.com/leynos/netsuke/issues/990)   | Unix backslashes become separators after a path passes validation.       | Preserve the checked path's host-native meaning through rebasing and serialization.                                   |
| [#980](https://github.com/leynos/netsuke/issues/980)   | An unqualified parent interpreter permits workspace-dependent selection. | Preserve an executable binding admitted by an explicit trusted resolution policy in every transport.                  |
| [#1017](https://github.com/leynos/netsuke/issues/1017) | A portable-word allowlist accepts PowerShell array separators.           | Preserve each admitted path as the intended argument value in every claimed shell context.                            |

_Table 1: Initial issue families and the contracts that outlive their fixes._

The inventory MUST include related mechanisms discovered during implementation.
Initial related cases include
[#967](https://github.com/leynos/netsuke/issues/967) on script transport,
[#971](https://github.com/leynos/netsuke/issues/971) on comment boundaries, and
[#1029](https://github.com/leynos/netsuke/issues/1029)
on PowerShell-to-native argument serialization. Reuse existing issues and
evidence where they already own the work.

### 2.2. Baseline and documentary authority

The source baseline is commit `e5d57c58352b51473de4170f86e34fdc9db3ece6`,
inspected on 2026-11-01. Implementation issues MUST refresh this inventory
against their own base revisions.

Current responsibilities span
[`cmd_interpolate`](../../src/ir/cmd_interpolate/mod.rs),
[`shell_word`](../../src/shell_word.rs),
[`recipe_shell`](../../src/recipe_shell.rs),
[`ninja_gen::recipe_shell`](../../src/ninja_gen/recipe_shell.rs), and the
[`glob` adapter](../../src/manifest/glob/mod.rs). The linter also has a
[`shellscan`](../../src/lint/rules/shellscan/mod.rs) module. Its advisory
analysis MUST NOT become a second authority for executable-recipe admission.

Existing decisions continue to govern until a follow-up ADR explicitly amends
them:

- [ADR-0014](../adrs/adr-0014-use-a-typed-ninja-text-escaping-seam.md) separates
  shell text from Ninja escaping.
- [ADR-0034](../adrs/adr-0034-preserve-script-in-and-out-as-shell-variables.md)
  preserves literal script `$in` and `$out` as shell variables.
- [ADR-0041](../adrs/adr-0041-canonical-recipe-shell-quoting-surface.md) owns
  the canonical recipe-shell quoting surface.
- [ADR-0047](../adrs/adr-0047-bound-kani-intermediate-representation-ir-harnesses-to-small-n.md)
  records production-connected Kani harnesses and their bounded scope.
- [RFC 0026](rfc-0026-harden-the-compiler-and-execution-boundaries.md) owns the
  broader resolved-operation and interpreter-binding direction. Reuse that
  ownership rather than introduce a competing executable plan.

The [formal-verification guide](../formal-verification-methods-in-netsuke.md)
currently limits Verus to a narrower optional kernel role. Adopting the
production-linked shell proofs proposed here requires an ADR that reconciles
that policy, its existing decision records, and the developer guidance before
making the new proof gates mandatory.

The policy ADR MUST identify the newly accepted kernel domains, their
production-linkage requirements, and their required workflow gates, and update
the formal-verification and developer guides accordingly.

### 2.3. Scope

The programme covers scalar commands, scripts, applicable structured-command
adapters, public library construction paths, and direct and response-file
transports. It includes byte and Unicode coordinate conversion where those
representations meet. It preserves deterministic output and established
placeholder semantics unless a reviewed compatibility decision changes them.

It does not verify an entire shell, add a shell runtime, or sandbox an authored
recipe. It does not implement the complete structured-argv programme from
[RFC 0001](rfc-0001-structured-command-blocks-and-argv-templates.md), introduce
new shell names outside [RFC 0011][rfc-0011], or require PowerShell on every
machine that generates a Windows-targeted build.

[rfc-0011]: rfc-0011-allow-listed-structured-command-shells.md

## 3. Trust boundaries and production contracts

The authored recipe supplies executable intent. Paths and other lower-trust
binding values supply data. The compiler MUST preserve that distinction through
validation and emission. Parsing does not confer authority on authored code,
and successful syntax validation does not prove that a rewrite preserved intent.

The proposed pipeline separates the following responsibilities. Role names in
this table specify contracts, not new public Rust API spellings.

| Boundary                    | Required result                                                                                   | Evidence or external assumption                                                      |
| --------------------------- | ------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| Recipe syntax               | Complete parse under an explicit dialect and options, with source provenance and insertion roles. | Qualified parser and target-interpreter agreement.                                   |
| Insertion admission         | Every binding passes the applicable shared context policy before emission.                        | Production gate checks and caller integration evidence.                              |
| Contextual encoding         | Accepted values retain their intended value and argument cardinality in the admitted context.     | Independent restricted semantics plus real-interpreter tests.                        |
| Source replacement          | Ordered, non-overlapping replacements preserve every untouched source byte.                       | Bounded checks and a production-linked sequence refinement.                          |
| Path representation         | Rebasing and serialization preserve the admitted host-native path meaning.                        | Pure preservation checks plus filesystem adapter tests.                              |
| Interpreter binding         | Every executable plan retains the resolver-admitted interpreter identity and spelling.            | Constructor and renderer checks; explicit resolver and operating-system assumptions. |
| Ninja and process transport | Each layer consumes the representation promised by the preceding layer.                           | Composition contracts and end-to-end invocation tests.                               |

_Table 2: Owned transformations and the assumptions that remain outside them._

Absolute paths and trusted executable bindings are distinct concepts. The
resolver MUST establish the permitted authority for an interpreter; a renderer
MUST NOT manufacture trust by making a workspace-supplied path absolute. The
binding MUST survive command and response-file selection without a bare-name
fallback. Filesystem replacement races and operating-system dispatch remain
explicit external contracts.

Paths, argument lists, shell fragments, complete scripts, and Ninja values MUST
retain separate representations at their boundaries. An encoder's output MUST
NOT serve as a universal safe string. Runtime syntax states belong in explicit
enums or equivalent data models. Checked construction can express lifecycle
invariants, but typestate alone cannot establish an input-dependent quote state.

## 4. Parser qualification

### 4.1. POSIX and Bash

`brush-parser` is the first production integration candidate because it offers
a standalone Rust parser and source-bearing word pieces for quotes, escapes,
and substitutions. The brush project reports a Bash comparison suite and parser
fuzzing. Those are reasons to qualify it, not a conformance certificate.[^1]

The qualification issue MUST record the resolved dependency version, licence,
required Rust toolchain compatibility, dialect options, source-coordinate
semantics, resource limits, and supported syntax. Cargo dependency requirements
MUST follow the repository's version policy; the lockfile and evidence record
identify the exact version tested. Dependency updates MUST rerun the contract
corpus before adoption.

The adapter MUST retain the original source and marker provenance. It MUST map
markers to complete syntactic roles, including surrounding word pieces and
active escapes. A decoded token spelling alone cannot recover that information.
Character, UTF-8 byte, and PowerShell UTF-16 positions MUST remain
distinguishable; the adapter MUST validate conversions before slicing source.
Brush documents its source index in character terms.[^2]

Qualification MUST establish provenance through tokenizer normalization and
secondary word parsing, including removed line continuations, nested source
strings, and nodes with unavailable locations. It MUST account for every
insertion-relevant source range. Missing provenance MUST reject the affected
insertion or require an upstream or parser-selection change. Netsuke MUST NOT
introduce another shell grammar to reconstruct missing spans.

The [existing heredoc tests](../../src/ir/cmd_interpolate/tests.rs) support
marker-derived delimiters. A template containing an unexpanded marker may not
form a complete shell program when its terminator depends on the binding value.
The qualification ADR MUST either define a justified staged parse and admission
procedure for that role or explicitly deprecate or reject it with migration
guidance. Binding data MUST NOT acquire source authority merely to obtain an
AST. The migration cannot assume that a complete parse of marker-filled source
succeeds for every currently accepted template.

Use `mvdan/sh` as an independent syntax comparison implementation in continuous
integration (CI). Its explicit dialect selection and full-program parser
provide a useful second view. Qualification MUST disable optional recovery and
early termination; disagreement requires investigation against the applicable
grammar and interpreter, rather than majority voting among parsers.[^3]

The acceptance corpus MUST cover the named issue families, nested
substitutions, quoted and unquoted words, comments, heredocs, escape parity,
Unicode, line endings, truncation, and every supported placeholder role.
Require complete input consumption, useful source positions, and no unnoticed
recovery. Record parser depth, input-size, time, and allocation behaviour
against declared budgets. Grammar-affecting shell options and unsupported
dynamic constructs MUST have an explicit policy; parser acceptance MUST NOT
imply arbitrary runtime environment equivalence.

### 4.2. PowerShell

Microsoft's `Parser.ParseInput` returns an abstract syntax tree (AST), tokens,
and parse errors. The Windows reference helper MUST use the target PowerShell
version and reject a non-empty error array even when an AST exists.[^4]

Begin with an isolated CI oracle. Launch a trusted absolute interpreter without
profiles, use a fixed helper, and pass candidate source through a data channel.
The helper MUST parse rather than invoke or compile the candidate script. Bound
input, output, runtime, and diagnostics, and qualify parser-side behaviour for
the supported constructs. Treat helper failures and unavailable interpreters as
missing evidence, not successful validation.

Windows PowerShell 5.1 and newer `pwsh` versions MUST have separate evidence
rows where both are supported. A production helper or alternative grammar needs
an ADR covering deployment, cross-target generation, version selection,
isolation, and failure behaviour before adoption. This RFC introduces neither
as an unconditional runtime dependency.

The corpus MUST exercise cmdlets or functions as well as native applications.
PowerShell gives commas different argument semantics for those consumers, and
its native invocation adds a serialization boundary beyond string-literal
encoding.[^5] A native argument recorder alone cannot establish the cmdlet case.

Parser output alone MUST NOT establish whether a command resolves to a
function, cmdlet, alias, or native application. Consumer-specific guarantees
MUST state and test that resolution assumption.

### 4.3. Alternatives

Tree-sitter grammars can supply useful syntax structure, but adapters must
reject error and missing nodes and establish target-dialect coverage before
using them for admission.[^6] `yash-syntax` offers a POSIX-oriented Rust parser
under GPL-3.0-or-later, which requires an explicit dependency decision.[^7]
Word splitters such as `shell-words` do not implement the full script grammar
required to classify these insertion sites.[^8]

Retaining the handwritten scanner as the permanent authority keeps the grammar
maintenance burden in Netsuke. Replacing it with parser-combinator machinery
would change implementation technique without transferring that responsibility.
Parser qualification may reject the first candidate; the follow-up ADR MUST
record the evidence and selected alternative without weakening the contracts.

## 5. Validation and emission

### 5.1. Admit supported insertion sites

The implementation MUST preserve internal placeholder identity through template
rendering and source analysis. Authored lookalikes, literal shell variables,
comments, and protected source MUST NOT become new binding sites. The adapter
MUST account for marker spelling, source coverage, and surrounding syntax
rather than rediscover markers by searching a decoded AST.

Every provenance-tracked marker occurrence MUST have one recorded disposition:
admitted replacement, preservation in a designated literal region, or
rejection. An ordinary argument, command position, assignment value,
redirection operand, pattern, arithmetic expression, and heredoc delimiter MUST
NOT be treated as interchangeable word roles.

A shared admission function MUST distinguish whole arguments, supported word
fragments, and forbidden or unsupported roles. Prefer whole-argument insertion.
Retain existing fragment support only with a stated contextual contract and
tests for prefix and suffix composition. The immediate active-escape rejection
from #984 MUST remain effective until the new path demonstrably subsumes it.

No accepted path may insert a binding in a protected heredoc body or
unsupported delimiter position. Unknown roles, parser errors, exhausted
resource budgets, invalid coordinates, and unsupported syntax MUST stop that
transformation before it produces an executable plan. Parser failure MUST NOT
select a more permissive legacy scanner.

### 5.2. Preserve source and values

One production replacement function SHOULD own ordered source-span splicing.
Its contract MUST establish valid source boundaries, non-overlap, exact copying
outside replacement spans, exact placement of approved replacements, and
checked output-size arithmetic. Reprinting a complete AST is not the default
migration strategy because it expands the behavioural surface under review.

For a whole-argument binding, the contextual encoder's contract is that
decoding its result under the declared restricted dialect and context yields
exactly the intended argument value. List insertion additionally specifies
cardinality and ordering, including empty-list behaviour. Fragment insertion
needs a separate contract for composition with the retained source. A
successful reparse of the result is supplementary evidence; altered command
structure can remain valid syntax.

The path adapter MUST preserve the literal Unix suffix after intended base
removal. A template policy may reject an unsupported spelling, but MUST NOT
rewrite it into a different accepted path. The portable unquoted-path policy
MUST name its admitted contexts and independent language specification. A
per-character allowlist proof alone does not establish argument position or
consumer behaviour.

### 5.3. Preserve execution and transport boundaries

Shell quoting and Ninja escaping MUST remain separate, ordered transformations
under ADR-0014 and ADR-0041. Direct and response-file routes MUST share the
same validated recipe and interpreter binding. Every additional encoding or
decoding layer needs an explicit input/output contract and an integration test.

The Windows native-child contract MUST state the supported argument convention.
Windows delivers a command-line string, and child applications may interpret it
differently; a correct PowerShell string value does not itself prove the native
argument vector.[^9] Structured argv execution remains a separate RFC 0001
capability that may reuse these contracts without blocking this programme.

Diagnostics MUST identify the dialect, failure category, and available source
location. Preserve existing localization and public error contracts where
possible. Unexpected parser failures MUST remain distinguishable from supported
syntax rejected by policy; avoid logging complete recipe or binding contents
when a bounded diagnostic suffices.

## 6. Verification strategy

### 6.1. Retain independent behavioural evidence

Regression and Proptest suites remain mandatory. Add differential cases against
the supported interpreters and parser references. Run controlled fixtures in
isolated processes with injected environments and bounded resources, following
the repository's test-isolation rules. Observe exact arguments, preserved
script bytes, rejection behaviour, and expected controlled effects as
appropriate.

Injection regressions MUST also assert the absence of the seeded unintended
effect, rather than checking only expected argument values or exit status.

Every historical regression MUST remain attributable to its origin after files
move or helpers merge. Generate inputs around boundaries and combinations, not
only the originally reported spelling. Test direct and response-file routes,
command and script traversal, and applicable public library entry points.

### 6.2. Kani checks over production kernels

The existing [interpolation harnesses][interpolation-proofs] prove marker
recognition in eight- and 32-character domains. They do not prove the complete
scanner. ADR-0047 and the [implementation record][kani-plan] document the full
scanner's resource limit and the complementary Proptest coverage.

New bounded checks MUST call production functions. Prioritize admission-state
combinations, encoder selection, source-span replacement, path preservation,
portable-word acceptance, interpreter binding, transport selection, and size
arithmetic. Retain a separate exact-marker theorem where transition checks use
marker events. The running adapter and proof must share that event boundary, or
a refinement must connect them.

Every harness MUST declare its value domain, length and nesting bounds,
unwinding bounds, assumptions, expected production symbols, and resource cap.
Retain unwinding assertions. Distinguish a counterexample from insufficient
unwinding, unsupported operations, timeout, and resource exhaustion; none of
the latter outcomes establishes the property.[^10]

The current symbolic-character helper maps bytes to characters and therefore
covers U+0000 to U+00FF. Unicode-sensitive claims MUST quantify over valid
Unicode scalars or justify a reduced alphabet. Any abstraction MUST preserve
lexical class, delimiter equality, escape behaviour, and spelling where
identity matters. A single representative for all other characters can
incorrectly merge different delimiters.

Unicode-scalar claims do not cover arbitrary Unix path bytes or unpaired
Windows UTF-16 code units. Each path adapter MUST state its admitted encoding
domain and preserve unsupported native representations losslessly or reject
them explicitly without lossy conversion. This requirement does not expand the
current UTF-8 path interface.

Use the repository's pinned Kani workflow and resource wrapper. Refactor the
owned pure boundary when a broad harness is intractable; do not raise resource
caps or narrow assumptions silently. Kani's experimental loop contracts offer
another possible inductive route, but adoption requires a separate tooling and
evidence decision.[^11]

[interpolation-proofs]: ../../src/ir/cmd_interpolate/verification.rs
[kani-plan]: ../execplans/4-2-3-kani-harnesses-for-command-interpolation.md

### 6.3. Verus refinements attached to production

After the policy ADR, the programme MUST deliver production-linked proofs for
the selected preservation and composition kernels. Initial obligations cover
source-span replacement, path serialization, insertion admission plus encoding,
and preservation of interpreter bindings. The ledger MUST distinguish a proved
local property from an unproved caller or transport connection.

Follow mdtablefix's [verified-core decision][md-adr]: include the production
function body directly, or prove a refinement to it. Shared executable bodies
with proof annotations in the verifier build are acceptable. A separate model
that merely resembles production cannot establish a production guarantee.
mdtablefix's [entry point][md-verus] and [claim ledger][md-ledger] demonstrate
the linkage and explicit accounting of remaining external contracts.

Specifications MUST independently express the required behaviour. A gate's
postcondition MUST constrain the actual acceptance and emission path, including
the caller that establishes the encoder's preconditions. Sequence proofs MUST
state their capacity and representation assumptions. If termination matters,
record whether the theorem establishes it as well as partial correctness.

A required production-to-proof linkage or owned-stage composition obligation
MUST NOT be discharged merely by recording it as an external assumption.
Reducing such an obligation requires an explicit reviewed RFC amendment.

The parser, interpreter, Ninja, operating system, and filesystem remain
external unless a separate proof establishes their behaviour. Verus assumptions
and `external_body` declarations MUST appear in the ledger and MUST NOT assume
the property being proved.[^12] Real-interpreter tests provide independent
evidence for these contracts; they do not turn an external component into a
proved one.

[md-adr]: https://github.com/leynos/mdtablefix/blob/4a9bdf0e7b061d772ba4d11668e935e1c38e8300/docs/adrs/0011-verified-normalization-core.md
[md-verus]: https://github.com/leynos/mdtablefix/blob/4a9bdf0e7b061d772ba4d11668e935e1c38e8300/verus/lib.rs
[md-ledger]: https://github.com/leynos/mdtablefix/blob/4a9bdf0e7b061d772ba4d11668e935e1c38e8300/docs/verification.md

### 6.4. Require evidence that each check matters

Each new proof obligation MUST have positive witnesses or reachability checks
and a negative control that changes relevant production behaviour while leaving
the specification intact. Suitable mutations restore an active-escape bypass,
broaden a delimiter boundary incorrectly, replace Unix backslashes, admit a
comma, or substitute a bare interpreter in one transport.

A mutation counts only when the changed program compiles, the intended harness
or theorem runs, and the expected semantic assertion or postcondition fails.
Missing tools, zero selected proofs, syntax errors, unrelated failures, and
timeouts MUST be reported as inconclusive or broken evidence. Restore
production and require the same obligation to pass.

The verification ledger MUST record, for each claim: production symbol and
callers; source revision; dialect and context; input domain and bounds;
specification; toolchain and tool versions; external assumptions; positive and
negative controls; actual result and evidence location; and remaining caller or
transport gaps. Required CI jobs MUST fail on missing expected harnesses or
proofs and MUST distinguish unavailable platform evidence from a pass.

## 7. Compatibility and migration

The fixes for the originating issues establish the minimum safety baseline.
Implementation MUST preserve their regression coverage throughout migration.
Existing accepted inputs, graph identity, Ninja output, and diagnostics need
characterization before switching an authority. Any intentional rejection or
behaviour change needs an explicit compatibility decision and the applicable
user and migration documentation.

New parser-backed analysis may initially run in comparison tests while the
fixed production path remains authoritative. Promotion requires the declared
qualification corpus and proof obligations for that slice to pass. A diagnostic
or opt-in linter mode MUST NOT weaken executable-recipe validation. After a
slice migrates, parser errors MUST NOT fall back to its old permissive path.

Maintainers may remove an immediate defensive check only when the shared path
subsumes its contract and preserves its regression evidence. They may retain a
check as deliberate defence in depth only with a documented owner, shared
contract, and corresponding tests. Independent duplicate classifiers and
untracked bypasses are not acceptable residual implementations.

The programme carries no release date or assumed v0.1.1 commitment. Immediate
release decisions remain with the originating defects and the existing release
process. RFC acceptance establishes follow-up work, not completion of that work.

## 8. Mandatory follow-up programme

### 8.1. Create tracking only after merge

After this RFC merges, the accepting maintainer MUST raise one tracking
meta-issue, provisionally titled **Implement RFC 0036: harmonize shell parsing
and verification**, and the implementation issues below. The RFC PR MUST NOT
create those issues in advance, use auto-closing references for the originating
bugs, or treat an RFC merge as their remediation.

The meta-issue MUST link the merged RFC revision, name an accountable programme
owner, enumerate the child issues and dependencies, and carry the harmonization
register and closure checklist. Creating the programme does not wait for every
immediate fix: each migration issue depends on the fixes relevant to its own
boundary. The original issues remain independently closable on their existing
acceptance criteria.

Harmonization alone MUST NOT reopen an originating defect or broaden its
acceptance criteria. Track residual migration work in separately linked
follow-up issues.

SP-01 to SP-09 below are proposal-local work identifiers, not allocated GitHub
issue numbers. Maintainers MUST replace them with links in the tracker, reuse
existing implementation issues where appropriate, and split work into focused
changes without dropping any outcome.

| Work                                              | Required deliverable and acceptance                                                                                                                                                                        | Dependencies                                                          |
| ------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------- |
| SP-01: Inventory fixes and ratify contracts       | Register all named and related fixes, entry points, regressions, owners, and residual bypasses; record required ADRs and the verification-policy amendment.                                                | RFC merge.                                                            |
| SP-02: Qualify parser and dialect adapters        | Run the corpus against pinned candidates and interpreters; record accepted syntax, coordinates, limits, platform coverage, and the parser-selection ADR.                                                   | SP-01.                                                                |
| SP-03: Establish shared production contracts      | Introduce or reuse provenance, admission, replacement, encoding, path, and binding seams; record ownership and a proof ledger with no copied production models.                                            | SP-01; coordinate parser contracts with SP-02.                        |
| SP-04: Migrate POSIX and Bash traversals          | Route applicable command, script, comment, and heredoc handling through qualified analysis and shared gates; retain all originating regressions.                                                           | SP-02, SP-03, and relevant immediate fixes.                           |
| SP-05: Qualify PowerShell boundaries              | Add the version-matched oracle and cmdlet/native corpus; harmonize accepted insertion roles, encoding, and native argument contracts; record any production-parser ADR.                                    | SP-02, SP-03, and relevant immediate fixes.                           |
| SP-06: Harmonize paths and executable binding     | Preserve path identity and portable-word semantics; carry trusted interpreter bindings through every applicable direct and response-file route.                                                            | SP-03 and relevant immediate fixes; coordinate with SP-04 and SP-05.  |
| SP-07: Add bounded production checks              | Complete Kani obligations, domain declarations, positive witnesses, and semantic mutation evidence for each delivered kernel and adapter connection.                                                       | SP-03; incrementally follow SP-04 to SP-06.                           |
| SP-08: Prove preservation and composition         | Complete selected Verus refinements over production and discharge required caller and transport obligations; record external-component contracts separately and reconcile the ledger and proof-policy ADR. | Stabilized contracts from SP-03 to SP-06 and relevant SP-07 evidence. |
| SP-09: Retire duplicate logic and close the audit | Remove superseded scans and encoders, audit every public and internal route, update documentation and CI, and satisfy every tracker closure condition.                                                     | SP-04 to SP-08.                                                       |

_Table 3: Required programme outcomes and their sequencing._

SP-04 to SP-06 may prepare candidate integrations before their verification
work completes. Promotion of each slice to production authority requires its
applicable SP-07 and SP-08 evidence. The dependencies describe an incremental
sequence, not permission to switch authority before verification.

Each child issue MUST state its originating issues and fix commits, affected
production entry points, applicable contracts, acceptance evidence,
dependencies, and any ADR required before implementation. The programme MUST
reconcile its tasks with RFC 0026 and the structured-command work instead of
scheduling a second owner for the same boundary.

### 8.2. Harmonize every ad hoc fix

The tracker MUST maintain one register entry per immediate fix or related
mitigation. Each entry records:

- Originating issue, fix pull request, and merged commit.
- Original regression cases and affected command, script, library, and
  transport paths.
- Canonical contract and final production owner under this RFC.
- Follow-up implementation issue, relevant ADR, and migration pull request.
- Property, differential, Kani, and Verus evidence applicable to that contract,
  including explicit external assumptions and gaps.
- Final disposition: subsumed by the shared path, or retained as documented
  defence in depth under that same contract.

The five named issues MUST each have an entry even if their fixes have already
merged or use different modules by the time the tracker opens. Later emergency
fixes in the covered boundaries MUST join the register. No entry may close
solely because its originating bug closed, a parser dependency landed, or a
test moved.

### 8.3. Record architectural decisions when they become concrete

Follow-up issues MUST raise or amend ADRs for substantive choices: parser and
dialect selection; admitted syntax and contextual encoding; interpreter trust
and transport policy; production-linked proof architecture and required gates;
and observable compatibility changes. Reuse an existing ADR when an amendment
preserves its historical decision; create a new one when the choice has a
distinct scope.

Each ADR MUST link this RFC, its implementation issue, the evidence informing
the decision, consequences, and migration obligations. Use the repository's
[documentation style guide](../documentation-style-guide.md), including status
and amendment conventions. An ADR cannot waive a required harmonization outcome
silently; a changed requirement needs an explicit reviewed RFC amendment and
corresponding tracker update.

### 8.4. Meta-issue closure conditions

The programme owner may close the meta-issue only when:

- Every named and subsequently registered fix has a completed harmonization
  entry and traceable regression evidence.
- Every applicable production entry point uses its canonical contracts, and
  the audit accounts for retained checks, alternate constructors, and both
  transport routes.
- Parser qualification covers the declared dialect and platform matrix;
  unsupported cases reject explicitly and unavailable evidence is visible.
- Required property tests, interpreter comparisons, Kani checks, Verus proofs,
  and semantic negative controls pass for the recorded source revision.
- The ledger distinguishes bounded checks, unbounded refinements, sampled
  tests, and external assumptions, with no outstanding required
  production-to-proof linkage or owned-stage composition gap.
- Superseded logic has been removed or justified under the shared contracts;
  all required ADRs and the design, developer, user, and migration
  documentation reflect the delivered behaviour.

Writing the child issues or merging the parser adapter is not programme
completion. Unfinished required work keeps the tracker open unless maintainers
explicitly amend the RFC and account for the changed scope.

## 9. Outstanding decisions and recommendation

Qualification must settle the parser version and supported dialect options,
PowerShell production analysis strategy, initial accepted insertion-role set,
resource budgets, and the exact proof-compatible production boundaries. The
corresponding child issue and ADR own each decision. No implementation result
or performance measurement is implied by this RFC.

Accept the common contracts and mandatory harmonization process. Deliver the
immediate fixes independently, then replace duplicated grammar ownership and
prove the small transformations that remain. Retain real-interpreter evidence
at every external semantic boundary.

[^1]: [brush project, independent parser, and testing](https://github.com/reubeno/brush);
    [word parsing API](https://docs.rs/brush-parser/latest/brush_parser/word/index.html).

[^2]: [Brush source positions](https://docs.rs/brush-parser/latest/brush_parser/struct.SourcePosition.html)
    and [parser options](https://docs.rs/brush-parser/latest/brush_parser/struct.ParserOptions.html).

[^3]: [`mvdan/sh` syntax API](https://pkg.go.dev/mvdan.cc/sh/v3/syntax),
      including
    `Variant`, `Parse`, `RecoverErrors`, and `StopAt`.

[^4]: [Microsoft PowerShell `Parser.ParseInput`](https://learn.microsoft.com/en-us/dotnet/api/system.management.automation.language.parser.parseinput?view=powershellsdk-7.4.0).

[^5]: [Windows PowerShell 5.1 parsing and native argument rules](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.core/about/about_parsing?view=powershell-5.1).

[^6]: [Tree-sitter node error and missing-node APIs](https://docs.rs/tree-sitter/latest/tree_sitter/struct.Node.html).

[^7]: [`yash-syntax` package metadata](https://docs.rs/crate/yash-syntax/latest/source/Cargo.toml).

[^8]: [`shell-words` supported and unsupported syntax](https://docs.rs/shell-words/latest/shell_words/fn.split.html).

[^9]: [Rust process argument splitting on Windows](https://doc.rust-lang.org/std/process/index.html#windows-argument-splitting).

[^10]: [Kani loop unwinding and bounded checks](https://model-checking.github.io/kani/tutorial-loop-unwinding.html).

[^11]: [Kani experimental loop contracts](https://model-checking.github.io/kani/reference/experimental/loop-contracts.html).

[^12]: [Verus assumptions and trusted components](https://verus-lang.github.io/verus/guide/tcb.html).
