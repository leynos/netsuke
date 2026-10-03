"""Implement the RFC 0005 release-admission gate as importable Python.

The package mirrors the boundary the Bash gate drew for itself, and each module
owns exactly one of them:

- ``policy`` owns the pure bounded classifications and the fixed vocabularies.
- ``records`` renders a validated value into the JSON Lines text a reader sees.
- ``delivery`` writes that text to a file, an adapter, or standard output.
- ``commands`` runs every external command and reports what the system said.
- ``telemetry`` decides whether and where a record is written, and what a
  failed write costs the gate.
- ``gate`` owns orchestration and configuration, and is the composition root.

Splitting them keeps each module small enough to read and lets the runtime
tests mock a single boundary at a time.

The entry point is the ``release_admission`` module beside this package; it
owns the Cyclopts surface and the process exit status.
"""
