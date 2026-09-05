from __future__ import annotations

from collections.abc import Iterable


def hierarchical_byte_deletions(value: bytes) -> Iterable[bytes]:
    """Yield deterministic strict-subset byte candidates from coarse to fine.

    Each pass partitions the current byte string into contiguous chunks and yields
    candidates with one chunk removed. Chunk widths descend by powers of two until
    single-byte deletion is reached. Duplicate candidates are suppressed while
    preserving first-seen order, which gives ``reduce_case`` a stable candidate
    contract without embedding domain semantics in the generic reducer.
    """
    length = len(value)
    if length == 0:
        return

    width = 1 << (length.bit_length() - 1)
    seen: set[bytes] = set()

    while width >= 1:
        for start in range(0, length, width):
            candidate = value[:start] + value[min(start + width, length) :]
            if candidate == value or candidate in seen:
                continue
            seen.add(candidate)
            yield candidate
        width //= 2
