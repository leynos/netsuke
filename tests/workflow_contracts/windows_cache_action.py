"""Read the Windows gate cache action from disk.

The one place the Windows cache contracts touch the filesystem or the YAML
parser, so their query helpers stay pure with respect to both. Kept beside
those contracts rather than inside them because two test modules need it and
neither should own a read the other depends on.

Every failure names the file and the operation that failed. "No such file or
directory" surfacing from inside an ownership assertion tells a reader nothing
about which of several YAML inputs went missing.
"""

import typing as typ

import yaml
from workflow_loading import REPO_ROOT, require_mapping

if typ.TYPE_CHECKING:  # pragma: no cover - imported for annotations only
    import pathlib as pl

#: The composite action whose save conditions the ownership contracts read.
ACTION_PATH = REPO_ROOT / ".github" / "actions" / "windows-gate-cache" / "action.yml"


def load_cache_action(path: pl.Path) -> dict[str, object]:
    """Read and parse the Windows cache action at `path`.

    The only place in this module that touches the filesystem or the YAML
    parser, so the query helpers above stay pure with respect to both. Each
    failure names the file and the operation that failed, because "No such file
    or directory" from inside an ownership assertion tells a reader nothing
    about which of several YAML inputs went missing.

    Parameters
    ----------
    path:
        The composite action to read.

    Returns
    -------
    dict[str, object]
        The parsed action mapping.

    Raises
    ------
    RuntimeError
        If the file cannot be read, or its contents are not valid YAML.
    """
    try:
        text = path.read_text(encoding="utf-8")
    except OSError as error:
        message = f"could not read the Windows cache action at {path}: {error}"
        raise RuntimeError(message) from error
    try:
        parsed = yaml.safe_load(text)
    except yaml.YAMLError as error:
        message = f"could not parse the Windows cache action at {path}: {error}"
        raise RuntimeError(message) from error
    return require_mapping(parsed, str(path))
