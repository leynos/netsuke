"""Read a downloaded archive under a decompressed-byte budget.

A gzip bomb is small to store and unbounded to expand: a few kilobytes of
repetition becomes gigabytes. Capping the compressed download therefore bounds
neither the memory the reader holds nor the work a hostile archive can demand,
and ``tarfile`` reads the stream as it parses — including the extended-header
bodies that carry long member names, which it must decode before it can produce
a member record.

This module puts the cap where the bytes are actually produced, and gives the
tar reader the one thing it needs from a file object besides ``read``: a
backward ``seek``, which opens a second bounded pass over the same download.
Reads are served in chunks, so a header that declares a huge size cannot make
the reader allocate that size before the cap has refused it, and
:meth:`InflatedStream.drain` reads a pass to its end so gzip verifies the
trailer that the tar end marker leaves unread. The archive's own refusals stay
in :mod:`release_binstall_archive`; the error raised at the cap is supplied by
the caller so a refusal keeps that identity.

Run through ``scripts/release_binstall_archive.py``; behavioural coverage lives
in ``scripts/tests/test_release_binstall_archive.py``.
"""

import gzip
import io
import typing as typ

if typ.TYPE_CHECKING:
    import collections.abc as cabc

#: Most decompressed bytes one pass over a download may produce. A released
#: archive holds a single-digit-megabyte binary, so four times the compressed
#: cap leaves a wide margin while holding a bomb to a few hundred megabytes of
#: reading instead of the disk-free gigabytes it would otherwise cost.
MAX_INFLATED_BYTES = 4 * 64 * 1024 * 1024

#: Bytes served at a time. Seeking forward reads in these steps so skipping an
#: offset does not allocate a buffer the size of the distance skipped, and
#: :meth:`InflatedStream.read` serves a request in them so a huge request is
#: never allocated before the budget has refused it.
CHUNK_BYTES = 64 * 1024


class InflatedStream(io.RawIOBase):
    """Serve a gzip stream to the tar reader under a decompressed-byte budget.

    Parameters
    ----------
    source
        The seekable compressed download, positioned at its first byte. The
        caller owns it; closing this stream leaves it open.
    limit
        Most decompressed bytes one pass may produce.
    refusal
        The exception to raise when the limit is reached. The caller supplies
        its own class so the refusal is the one its callers already catch.

    Notes
    -----
    ``tarfile`` walks the member index and then seeks back to the member it
    means to read, so a backward seek is ordinary rather than hostile. The
    download is seekable, so a backward seek opens a fresh decompressor at the
    first byte and skips forward; each pass carries its own budget, so reading
    a legitimate archive twice cannot exhaust it.

    The budget is measured in bytes served. A request larger than what remains
    is served only as far as the cap, and the refusal follows at once if the
    archive continues past it: a short read would otherwise be indistinguishable
    from the end of the stream, which ``tarfile`` reports as a corrupt archive
    rather than as an oversized one.

    Examples
    --------
    >>> import gzip
    >>> payload = gzip.compress(b"netsuke" * 4)
    >>> stream = InflatedStream(
    ...     io.BytesIO(payload), limit=1024, refusal=ValueError
    ... )
    >>> with stream:
    ...     stream.read()
    b'netsukenetsukenetsukenetsuke'
    """

    def __init__(
        self, source: typ.IO[bytes], *, limit: int, refusal: type[Exception]
    ) -> None:
        """Start decompressing ``source`` under ``limit`` decompressed bytes."""
        super().__init__()
        self._source = source
        self._limit = limit
        self._refusal = refusal
        self._reader = gzip.GzipFile(fileobj=source, mode="rb")
        self._inflated = 0
        self._exhausted = False

    @typ.override
    def readable(self) -> bool:
        """Report that this stream serves bytes.

        Returns
        -------
        bool
            Always ``True``, which routes ``RawIOBase.read`` through
            ``readinto``.
        """
        return True

    @typ.override
    def seekable(self) -> bool:
        """Report that this stream is not advertised as seeking.

        Returns
        -------
        bool
            Always ``False``. ``tarfile`` never asks; it calls ``seek``
            directly, and backwards only after it has walked the index.
        """
        return False

    @typ.override
    def tell(self) -> int:
        """Return the position within the decompressed archive.

        Returns
        -------
        int
            The bytes served in the current pass.

        Notes
        -----
        ``tarfile`` calls ``tell`` on the file object it is handed, so this
        has to be a method rather than a property.
        """
        return self._inflated

    @typ.override
    def read(self, size: int = -1) -> bytes:
        """Serve at most ``size`` bytes, allocating only what arrives.

        Returns
        -------
        bytes
            Up to ``size`` decompressed bytes, or everything left in this pass
            when ``size`` is negative. Fewer bytes mean the pass's budget or
            the archive ended first.

        Notes
        -----
        ``tarfile`` reads an extended header with the size that header claims,
        and the inherited ``read`` allocates that many bytes before
        :meth:`readinto` can refuse them. Serving the request in chunks keeps
        the allocation within what the budget admits, whichever interpreter
        the smoke runs on.

        The refusal raised at the cap is the caller's own class rather than one
        defined here; :meth:`_require_end` describes when it is raised.
        """
        if size < 0:
            return self._read_all()
        served = bytearray()
        while len(served) < size:
            chunk = bytearray(min(CHUNK_BYTES, size - len(served)))
            count = self.readinto(chunk)
            if count == 0:
                break
            served += chunk[:count]
        return bytes(served)

    def _read_all(self) -> bytes:
        """Read the rest of this pass in bounded chunks.

        Returns
        -------
        bytes
            Every byte left in this pass, which is at most the pass's budget.
        """
        served = bytearray()
        while True:
            chunk = bytearray(CHUNK_BYTES)
            count = self.readinto(chunk)
            if count == 0:
                return bytes(served)
            served += chunk[:count]

    def drain(self) -> None:
        """Read to the end of this pass so gzip checks its trailer.

        Notes
        -----
        ``tarfile`` stops at the archive's end marker, which leaves the
        decompressor short of the gzip trailer; only a read to the end of the
        compressed member makes gzip verify the CRC and length it recorded.
        A pass that runs out of budget first is refused exactly as a read is.
        """
        _ = self._read_all()

    def readinto(self, buffer: cabc.Buffer) -> int:
        """Serve decompressed bytes without passing the pass's budget.

        Returns
        -------
        int
            The bytes served, which is zero once the archive ends.

        Notes
        -----
        The refusal raised at the cap is the caller's own class rather than one
        defined here, so :func:`_require_end` is where it is described.
        """
        view = memoryview(buffer)
        if not view or self._exhausted:
            return 0
        allowed = self._limit - self._inflated
        if allowed <= 0:
            self._require_end()
            return 0
        count = self._reader.readinto(view[:allowed])
        self._inflated += count
        if count == allowed < len(view):
            # The request outran the budget. Refuse here rather than return a
            # short read the caller would take for the end of the archive.
            self._require_end()
        return count

    def _require_end(self) -> None:
        """Accept a spent budget only if nothing follows it.

        Notes
        -----
        When bytes do follow, the exception raised is ``refusal``, the class
        the caller passed to the constructor.
        """
        if self._reader.read(1):
            msg = (
                "the archive expands to more than the "
                f"{self._limit} bytes the smoke will read"
            )
            raise self._refusal(msg)
        self._exhausted = True

    def seek(self, offset: int, whence: int = io.SEEK_SET) -> int:
        """Move to ``offset``, decompressing again when it lies behind us.

        Returns
        -------
        int
            The new position within the decompressed archive.

        Raises
        ------
        io.UnsupportedOperation
            If the position is measured from the archive's end, or lies before
            its start, which this stream cannot serve.
        """
        if whence == io.SEEK_END:
            msg = "the decompressed archive cannot be measured from its end"
            raise io.UnsupportedOperation(msg)
        target = offset if whence == io.SEEK_SET else self._inflated + offset
        if target < 0:
            msg = f"the decompressed archive cannot seek to {target}"
            raise io.UnsupportedOperation(msg)
        if target < self._inflated:
            self._restart()
        self._discard_to(target)
        return self._inflated

    def _restart(self) -> None:
        """Decompress again from the first byte, renewing the budget."""
        self._source.seek(0)
        self._reader.close()
        self._reader = gzip.GzipFile(fileobj=self._source, mode="rb")
        self._inflated = 0
        self._exhausted = False

    def _discard_to(self, target: int) -> None:
        """Read forward to ``target`` without retaining the skipped bytes."""
        scratch = bytearray(min(CHUNK_BYTES, max(target, 1)))
        while self._inflated < target:
            want = min(len(scratch), target - self._inflated)
            if not self.readinto(memoryview(scratch)[:want]):
                break

    def close(self) -> None:
        """Close the decompressor, leaving the caller's download open."""
        self._reader.close()
        super().close()


__all__ = ["CHUNK_BYTES", "MAX_INFLATED_BYTES", "InflatedStream"]
