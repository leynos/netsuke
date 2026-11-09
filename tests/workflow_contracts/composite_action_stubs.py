"""Hold the tool stand-ins that more than one composite-action harness needs.

Each harness runs a composite action's real shell bodies as child processes, so
the tools those bodies call have to exist. ``uv`` is the one both actions share:
the publisher invokes its CLIs and the verifier invokes its own through
``uv run --no-project --python <baseline>``, and neither prefix differs.

Keeping one copy here is what stops the two harnesses drifting into asserting
different prefixes for the same tool. A stand-in that merely existed twice would
still let the publisher's spelling and the verifier's spelling diverge, and the
interpreter pin could then be enforced in one suite and invisible in the other.

Run via ``make test-workflow-contracts``.
"""

#: Stands in for ``uv run --no-project --python <baseline>``, which fetches the
#: baseline interpreter on demand. The subject is the action's own script, so
#: the prefix is checked as spelled and the remainder is handed to the caller's
#: interpreter. Nothing downloads, so nothing needs the network.
UV_STUB = """\
#!/bin/sh
set -eu
test "$1" = run
test "$2" = --no-project
test "$3" = --python
printf '%s\\n' "$4" >> "$UV_BASELINE_LOG"
shift 4
exec "$UV_PYTHON" "$@"
"""

__all__ = ["UV_STUB"]
