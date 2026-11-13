"""Record what one producer built, from the bytes it is about to hand on.

Each release producer runs after packaging and before upload. At that moment
it alone holds the archive, its sidecar, and the operating-system packages it
just built, and it is the only process that can say which commit produced
them. This CLI exposes that as one command: it parses the claimed identity and
the artefacts to hash, hands the reading work to
:mod:`record_producer_provenance_record`, and maps each refusal to an exit
code the workflow can branch on.

The record is written beside the artefacts and travels to publication as part
of the same GitHub artefact. Publication compares the downloaded bytes against
it, so a substituted package, a replaced archive, or an artefact from an older
commit is visible even when a matching replacement sidecar was supplied.

Only files named explicitly on the command line are hashed. A tree walk here
would put retained workspace material and unrelated files into the record,
which is the opposite of what the record exists to prove: this producer is
answerable for these bytes and nothing else.

Run via ``.github/actions/upload-declared-assets``'s producers; behavioural
coverage lives in ``scripts/tests/test_record_producer_provenance_cli.py``.
"""

import argparse
import pathlib
import sys

from record_producer_provenance_errors import UnusableInputError
from record_producer_provenance_record import (
    RECORD_NAME,
    ProducerIdentity,
    record,
    recorded_assets,
    write_record,
)

__all__ = [
    "EXIT_OK",
    "EXIT_OUTPUT_UNWRITABLE",
    "EXIT_UNUSABLE_INPUT",
    "RECORD_NAME",
    "build_parser",
    "main",
]

EXIT_OK = 0
EXIT_UNUSABLE_INPUT = 1
EXIT_OUTPUT_UNWRITABLE = 2


def build_parser() -> argparse.ArgumentParser:
    """Build the parser for the producer provenance recorder.

    Returns
    -------
    argparse.ArgumentParser
        A parser requiring the candidate, run identity, target and platform,
        the artefacts to hash, and the record's destination.

    Examples
    --------
    >>> build_parser().parse_args([
    ...     "--commit", "a" * 40, "--run-id", "7", "--run-attempt", "1",
    ...     "--target", "linux-x86_64", "--platform", "linux",
    ...     "--asset", "a.deb", "--output", "record.json",
    ... ]).run_attempt
    '1'
    """
    parser = argparse.ArgumentParser(
        prog="record_producer_provenance_cli",
        description="Record the candidate and digests one producer built.",
    )
    parser.add_argument("--commit", required=True, help="the candidate commit")
    parser.add_argument("--run-id", required=True, help="the workflow run identity")
    parser.add_argument("--run-attempt", required=True, help="the run attempt")
    parser.add_argument("--target", required=True, help="the staged target key")
    parser.add_argument("--platform", required=True, help="the target's platform")
    parser.add_argument(
        "--asset",
        required=True,
        action="append",
        type=pathlib.Path,
        help="one artefact to record; repeat for every artefact",
    )
    parser.add_argument(
        "--output",
        required=True,
        type=pathlib.Path,
        help="where to write the producer record",
    )
    return parser


def main(argv: list[str] | None = None) -> int:
    """Record the producer's artefacts, or report why they cannot be recorded.

    Parameters
    ----------
    argv
        Command-line arguments. Defaults to ``sys.argv[1:]``.

    Returns
    -------
    int
        ``EXIT_OK`` when the record was written, ``EXIT_UNUSABLE_INPUT`` when
        the claimed identity or an artefact is unusable, or
        ``EXIT_OUTPUT_UNWRITABLE`` when the record cannot be installed.

    Examples
    --------
    A missing artefact is refused before anything is written:

    >>> import tempfile
    >>> from pathlib import Path
    >>> with tempfile.TemporaryDirectory() as tmp:
    ...     main(["--commit", "a" * 40, "--run-id", "7", "--run-attempt", "1",
    ...           "--target", "linux-x86_64", "--platform", "linux",
    ...           "--asset", str(Path(tmp) / "absent.deb"),
    ...           "--output", str(Path(tmp) / "record.json")])
    1
    """
    arguments = build_parser().parse_args(argv)
    try:
        assets = recorded_assets(
            arguments.asset, platform=arguments.platform, target=arguments.target
        )
        mapping = record(
            ProducerIdentity(
                commit=arguments.commit,
                run_id=arguments.run_id,
                run_attempt=arguments.run_attempt,
                target=arguments.target,
                platform=arguments.platform,
            ),
            assets,
        )
        write_record(mapping, arguments.output)
    except UnusableInputError as refusal:
        print(f"producer provenance refused: {refusal}", file=sys.stderr)
        return EXIT_UNUSABLE_INPUT
    except OSError as failure:
        print(f"producer provenance output unwritable: {failure}", file=sys.stderr)
        return EXIT_OUTPUT_UNWRITABLE
    return EXIT_OK


if __name__ == "__main__":
    raise SystemExit(main())
