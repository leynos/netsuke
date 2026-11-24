"""Variable names the mutation caller reports but must never assign.

The caller's ``setup-commands`` prints the compiler-flag variables and the
linker opt-out, so a run log explains which source decided the linker. The same
names are forbidden as keys anywhere else in the workflow: Cargo takes an
assigned ``RUSTFLAGS`` over every ``rustflags`` table in
``.cargo/config.toml``, and an assigned ``NETSUKE_PLATFORM_LINKER`` opts the
worker out of the mold selection the configuration's ``linker`` key provides.

The names live in a helper rather than in one test module because two suites
read them: the caller's exact-``with:`` contract and the whole-workflow guard.
"""

#: The compiler-flag variables the caller reports and must never assign.
FLAG_VARIABLES = ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS")

#: The linker opt-out the release and coverage shapes set. The mutation worker
#: must not set it: it exists to hand those shapes the platform linker, and the
#: worker is a development build that must select the pinned mold.
LINKER_OPT_OUT = "NETSUKE_PLATFORM_LINKER"

#: Every variable the caller reports, in report order.
REPORTED_VARIABLES = (*FLAG_VARIABLES, LINKER_OPT_OUT)


def flag_override_problems(workflow: dict[str, object]) -> list[str]:
    """Return every place ``workflow`` sets a forbidden variable as a key.

    The whole-workflow walk is deliberate: each prohibition is on *any* scope,
    so a future job, step, or reusable-workflow argument cannot introduce an
    override in a corner the caller's exact-block assertion never reads. Keys
    are checked rather than values, because ``setup-commands`` prints the
    variables without assigning them.

    Parameters
    ----------
    workflow
        The parsed caller workflow to inspect.

    Returns
    -------
    list[str]
        One entry per offending key, naming the dotted path and the variable,
        or an empty list when the caller satisfies the invariant. An empty
        result is the passing outcome, so callers assert on truthiness rather
        than on a sentinel.
    """
    found: list[str] = []

    def walk(node: object, path: str) -> None:
        """Record each forbidden variable key under ``node``, recursing down."""
        match node:
            case dict():
                for key, child in node.items():
                    child_path = f"{path}.{key}"
                    if key in REPORTED_VARIABLES:
                        found.append(f"{child_path} (a {key} key)")
                    walk(child, child_path)
            case list():
                for index, child in enumerate(node):
                    walk(child, f"{path}[{index}]")

    walk(workflow, "workflow")
    return found
