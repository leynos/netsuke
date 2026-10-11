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
- ``filesystem`` performs the two filesystem effects start-up needs: make a
  parent directory, and create one file empty. It is the gate's only filesystem
  write, and it exists so that effect can be doubled rather than induced.
- ``configuration`` resolves everything the gate is told before it runs: the
  required variables, the defaults, and the two artefact paths. Resolution
  writes nothing; only :func:`~.configuration.prepare_destination` does, and it
  is handed the filesystem it performs them on.
- ``telemetry`` decides whether and where a record is written, and what a
  failed write costs the gate.
- ``gate`` owns orchestration, and is the composition root. It depends on
  ``configuration`` for one name, the resolved settings it runs against.

Splitting them keeps each module small enough to read and lets the runtime
tests double a single boundary at a time.

The entry point is the ``release_admission`` module beside this package; it
owns the Cyclopts surface, the start-up order, and the process exit status.

``configuration``, ``environment``, and ``filesystem`` are this package's port
and adapter layer. ``configuration`` names what the gate is told and performs
no effect at all; ``environment`` reads the values one variable at a time; and
``filesystem`` performs the two effects that are left. The three are separate
modules because the ceiling on Python module length is a repository rule rather
than a preference, and because the split is the same one the design draws: a
pure resolver, a reader of the ambient source it resolves from, and an adapter
for the only system the gate writes to.
"""
