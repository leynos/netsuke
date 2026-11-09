"""Normalise a job's ``needs`` declaration into the names it lists.

GitHub accepts either a single job name or a list of names for ``needs``, and
the release-publication contracts compare those declarations against what each
publisher requires. Every reader must interpret the value the same way, or one
contract accepts a shape another refuses, so the interpretation lives here:
a string is one name, a list of strings is those names, and anything else is
malformed.

Returning ``None`` for a malformed declaration, rather than raising, leaves
each caller free to report the violation in its own diagnostic.
"""


def job_names(declared: object) -> list[str] | None:
    """Return the job names ``declared`` lists, or ``None`` when malformed.

    Returns
    -------
    list[str] | None
        One name for a string, the listed names for a list of strings,
        otherwise ``None``.

    Examples
    --------
    >>> job_names("metadata")
    ['metadata']
    >>> job_names(["metadata", "publish-release"])
    ['metadata', 'publish-release']
    >>> job_names({"metadata": True}) is None
    True
    """
    match declared:
        case str() as name:
            return [name]
        case list() as items:
            return _listed_names(items)
        case _:
            return None


def _listed_names(items: list[object]) -> list[str] | None:
    """Return every string in ``items``, or ``None`` if one is not a string.

    Returns
    -------
    list[str] | None
        The names in listing order, or ``None`` when any item is not a string.
    """
    names: list[str] = []
    for item in items:
        match item:
            case str() as name:
                names.append(name)
            case _:
                return None
    return names
