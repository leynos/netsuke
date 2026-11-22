"""Implement the RFC 0005 release-admission gate as importable Python.

The package mirrors the boundary the gate draws for itself, and each module
owns exactly one of them:

- ``policy`` owns the pure bounded classifications and the fixed vocabularies.
- ``records`` renders a validated value into the JSON Lines text a reader sees.
- ``delivery`` writes that text to a file, an adapter, or standard output.
- ``bounding`` spawns one allowlisted command in a process group of its own and
  signals the descendants that remain in that group during cancellation.
- ``commands`` runs every external command and reports what the system said.
- ``statuses`` translates an exit status, an elapsed bound, or a refused launch
  into the gate's own command result. It is the only place a shell's status
  numbering is interpreted, and both ``commands`` and ``delivery`` call it.
- ``environment`` reads a variable in one of Bash's two default forms.
- ``downstream`` names the #598 downstream migration canaries, the one list
  the evidence producer, the verifier's required results, and the workflow
  contract all read.
- ``manifest`` decodes the candidate-bound document a producer published: its
  schema version, its fields, and the one refusal every unreadable document
  collapses to. It is pure, so the schema is testable without a file.
- ``evidence`` decides what that document licenses: whether its timestamps are
  fresh, and whether its identity and required results admit the candidate
  under release. The two are split because a schema change and an admission-rule
  change are different changes, and because each half then sits well inside
  the module-length ceiling.
- ``admission_evidence`` composes those two: it reads the manifest through the
  filesystem port, classifies its window, and re-reads the window at the
  admission decision, so the bound holds when the gate answers rather than only
  when it first looked. It is separate from ``gate`` because the evidence
  decisions had grown to a third of the orchestration module, and because the
  questions it answers are about the document rather than about the run.
- ``filesystem`` performs the filesystem effects the gate needs: make a parent
  directory, create one file empty, and read one file under a byte bound. It is
  the gate's only *start-up* write, and it exists so those effects can be
  doubled rather than induced. ``delivery`` writes files too, but only ever
  through a configured sink or its own in-process append, so its effects are
  named by the record it delivers rather than by a port of their own.
- ``configuration`` resolves everything the gate is told before it runs: the
  required variables, the defaults, and the two artefact paths. Its resolution
  functions perform no filesystem effect;
  :func:`~.configuration.prepare_destination` orchestrates them through the
  supplied port.
- ``telemetry`` decides whether and where a record is written, and what a
  failed write costs the gate.
- ``gate`` owns orchestration, and is the composition root. It depends on
  ``configuration`` for one name, the resolved settings it runs against, on
  ``filesystem`` for the port the evidence decisions read through, and on
  ``admission_evidence`` for those two decisions.

Splitting them keeps each module small enough to read and lets the runtime
tests double a single boundary at a time.

The entry point is the ``release_admission`` module beside this package; it
owns the Cyclopts surface, the start-up order, and the process exit status.

``configuration``, ``environment``, and ``filesystem`` are this package's port
and adapter layer. ``configuration`` names what the gate is told and its
resolution functions perform no effect at all; ``environment`` reads the values
one variable at a time; and ``filesystem`` performs the effects that are left.
The three are separate modules because the ceiling on Python
module length is a repository rule rather
than a preference, and because the split is the same one the design draws: a
pure resolver, a reader of the ambient source it resolves from, and an adapter
for the only system the gate writes to.
"""
