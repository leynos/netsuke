"""Hold the ``gh`` stand-ins the declared-asset harness installs.

The harness runs the action's real shell bodies as child processes, which means
the tools those bodies call have to exist. Both are stubbed at the *path*
boundary rather than patched in Python: the steps invoke ``uv run ...`` and
``gh release ...`` as commands, so a stand-in on ``PATH`` is the only way to
exercise the arguments as spelled. The ``uv`` prefix is not this action's own
contract, so it lives in ``composite_action_stubs`` beside the verifier's copy.

The ``gh`` stand-in is the interesting one. It keeps the release state in a
JSON file, so a case can describe a release that already holds assets, and an
upload *appends* the bytes it was handed to that same file. A stub that merely
exited zero would let the caller's post-upload verification read a release that
never received the asset: the very failure that check exists to catch. The
digests are computed from the bytes the transfer was given, so a payload
tampered with between selection and upload surfaces as a conflict rather than a
match.

Run via ``make test-workflow-contracts``.
"""

from composite_action_stubs import UV_STUB

#: Stands in for the GitHub CLI. Every invocation is recorded, before the
#: subcommand is dispatched, so a case can assert which transfers were sent and
#: with which flags even when the transfer itself fails.
GH_STUB = """\
#!/bin/sh
set -eu
printf '%s\\n' "$*" >> "$GH_LOG"
case "${1:-} ${2:-}" in
  "release view") exec "$GH_PYTHON" "$GH_HELPER" view "$GH_ATTACHED" ;;
  "release download") exit 1 ;;
  "release upload")
    shift 3
    exec "$GH_PYTHON" "$GH_HELPER" upload "$GH_ATTACHED" "$@"
    ;;
  *)
    printf 'unexpected gh invocation: %s\\n' "$*" >&2
    exit 1
    ;;
esac
"""

#: The stand-in's release state. Kept in Python rather than shell so the
#: listing is JSON-encoded exactly and each digest is computed from the bytes
#: the transfer was handed, rather than from the name a case expected.
GH_HELPER = '''\
"""Track the assets a stubbed release holds, and the digest of each."""

import hashlib
import json
import pathlib
import sys


def _attached(listing: pathlib.Path) -> dict[str, dict[str, object]]:
    """Return the listing's assets, keyed by name."""
    decoded = json.loads(listing.read_text(encoding="utf-8"))
    return {asset["name"]: asset for asset in decoded["assets"]}


def _store(listing: pathlib.Path, assets: dict[str, dict[str, object]]) -> None:
    """Write ``assets`` back as the listing a later view will serve."""
    ordered = [assets[name] for name in sorted(assets)]
    listing.write_text(json.dumps({"assets": ordered}), encoding="utf-8")


def _view(listing: pathlib.Path) -> int:
    """Print the listing as ``gh release view --json assets`` would."""
    print(json.dumps({"assets": list(_attached(listing).values())}))
    return 0


def _upload(listing: pathlib.Path, files: list[str]) -> int:
    """Attach each file, recording the digest of the bytes it was given."""
    assets = _attached(listing)
    for name in files:
        data = pathlib.Path(name).read_bytes()
        assets[pathlib.Path(name).name] = {
            "name": pathlib.Path(name).name,
            "digest": "sha256:" + hashlib.sha256(data).hexdigest(),
        }
    _store(listing, assets)
    return 0


def main(argv: list[str]) -> int:
    """Answer one ``gh release`` call from the on-disk listing."""
    command, listing, *rest = argv
    path = pathlib.Path(listing)
    if command == "view":
        return _view(path)
    return _upload(path, rest)


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
'''

__all__ = ["GH_HELPER", "GH_STUB", "UV_STUB"]
