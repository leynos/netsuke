"""Hold the ``gh api`` stand-in the inventory verifier's harness installs.

The verifier reaches GitHub three times: it lists the releases to find the one
draft carrying the tag, lists that release's assets, and downloads each
attachment by number. All three are ``gh api`` calls, so the stand-in has to
serve listings and bytes rather than a release subcommand.

The state file is what makes the stub honest. It is written by the harness from
the same description a case asserts against, so the listing the step reads and
the bytes it downloads come from one place; a stub that printed a fixed page
would let a case pass while the step read an asset the release never held. The
download also has to ask for ``application/octet-stream``: without that header
``gh api`` returns the JSON metadata of the asset, not its bytes, and a stub
that ignored the header would hide exactly that mistake.

Every invocation is recorded before it is dispatched, so a case can assert which
calls were made even when the call itself failed. The ``uv`` prefix the steps
share with the publisher lives in ``composite_action_stubs``.

Run via ``make test-workflow-contracts``.
"""

#: Stands in for ``gh api``. ``--method GET`` is required rather than assumed:
#: ``-f`` and ``-F`` imply a POST, and a stand-in that accepted any method would
#: let a write through unnoticed. The parsed call is handed to the helper, which
#: answers it from the on-disk release state.
GH_API_STUB = """\
#!/bin/sh
set -eu
printf '%s\\n' "$*" >> "$GH_LOG"
if [ "${1:-}" != "api" ]; then
  printf 'unexpected gh invocation: %s\\n' "$*" >&2
  exit 1
fi
shift
method=""
paginate="0"
slurp="0"
header=""
path=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --method) method="${2:-}"; shift 2 ;;
    --paginate) paginate="1"; shift ;;
    --slurp) slurp="1"; shift ;;
    -H) header="${2:-}"; shift 2 ;;
    *) path="$1"; shift ;;
  esac
done
if [ "$method" != "GET" ]; then
  printf 'gh api must be an explicit GET, got %s\\n' "${method:-<none>}" >&2
  exit 1
fi
exec "$GH_PYTHON" "$GH_HELPER" "$GH_STATE" "$paginate" "$slurp" "$header" "$path"
"""

#: Answers one parsed ``gh api`` call from the state file. Kept in Python rather
#: than shell so the listings are JSON-encoded exactly as GitHub reports them
#: and the attachment bytes are copied unchanged.
GH_API_HELPER = '''\
"""Serve the release and asset listings, and attachment bytes, from one state."""

import json
import pathlib
import sys

#: The header that makes ``gh api`` return an attachment's bytes.
OCTET_STREAM = "Accept: application/octet-stream"

#: The fields GitHub reports for an attachment, in listing order.
ASSET_FIELDS = ("id", "name", "state", "size")

#: The fields GitHub reports for a release, in listing order.
RELEASE_FIELDS = ("id", "tag_name", "draft")


def _fail(message: str) -> int:
    """Report an unusable call and return a failing status."""
    print(message, file=sys.stderr)
    return 1


def _listed(state: dict[str, object], fields: tuple[str, ...]) -> list[dict]:
    """Return the state's assets reduced to the fields a listing reports."""
    return [
        {field: asset[field] for field in fields} for asset in state["assets"]
    ]


def _releases(state: dict[str, object]) -> int:
    """Print the release pages ``--slurp`` wraps in one array."""
    listed = [
        {field: entry[field] for field in RELEASE_FIELDS}
        for entry in state["releases"]
    ]
    print(json.dumps([listed]))
    return 0


def _assets(state: dict[str, object], release_id: str) -> int:
    """Print the asset pages of the named release."""
    if release_id != str(state["release_id"]):
        return _fail(f"no release is numbered {release_id}")
    print(json.dumps([_listed(state, ASSET_FIELDS)]))
    return 0


def _download(state: dict[str, object], asset_id: str, header: str) -> int:
    """Write one attachment's bytes, or refuse a download the case failed."""
    if header != OCTET_STREAM:
        return _fail(f"an attachment download must ask for {OCTET_STREAM}")
    for asset in state["assets"]:
        if str(asset["id"]) != asset_id:
            continue
        if asset["download_fails"]:
            return _fail(f"simulated download failure for {asset['name']}")
        sys.stdout.buffer.write(pathlib.Path(asset["source"]).read_bytes())
        return 0
    return _fail(f"no asset is numbered {asset_id}")


def main(argv: list[str]) -> int:
    """Answer one ``gh api`` call from the on-disk release state."""
    state_path, paginate, slurp, header, path = argv
    state: dict[str, object] = json.loads(pathlib.Path(state_path).read_text())
    segments = path.split("/")
    if segments[:1] != ["repos"] or len(segments) < 4:
        return _fail(f"unexpected gh api path {path!r}")
    route = segments[3:]
    if route == ["releases"]:
        if (paginate, slurp) != ("1", "1"):
            return _fail("the release listing must be paginated and slurped")
        return _releases(state)
    if len(route) == 3 and route[0] == "releases" and route[2] == "assets":
        if (paginate, slurp) != ("1", "1"):
            return _fail("the asset listing must be paginated and slurped")
        return _assets(state, route[1])
    if len(route) == 3 and route[:2] == ["releases", "assets"]:
        return _download(state, route[2], header)
    return _fail(f"unexpected gh api path {path!r}")


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
'''

__all__ = ["GH_API_HELPER", "GH_API_STUB"]
