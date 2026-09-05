from collections.abc import Sequence


class DeterministicByteMutations:
    """Replayable corpus source yielding seeds followed by single-bit mutations.

    Cases are ordered by seed, byte offset, then bit index (least-significant
    first). The complete schedule is finite and addressable by zero-based index,
    making a failing case reproducible without shared RNG state.
    """

    __slots__ = ("_cases",)

    def __init__(self, seeds: Sequence[bytes], *, max_case_bytes: int = 4096) -> None:
        if max_case_bytes <= 0:
            raise ValueError("max_case_bytes must be positive")
        if not seeds:
            raise ValueError("seeds must be non-empty")

        cases: list[bytes] = []
        seen: set[bytes] = set()
        normalized: list[bytes] = []
        for seed in seeds:
            if not isinstance(seed, bytes):
                raise TypeError("seeds must contain bytes")
            if len(seed) > max_case_bytes:
                raise ValueError("seed exceeds max_case_bytes")
            normalized.append(seed)
            if seed not in seen:
                cases.append(seed)
                seen.add(seed)

        for seed in normalized:
            for offset in range(len(seed)):
                for bit in range(8):
                    mutated = bytearray(seed)
                    mutated[offset] ^= 1 << bit
                    case = bytes(mutated)
                    if case not in seen:
                        cases.append(case)
                        seen.add(case)

        self._cases = tuple(cases)

    @property
    def case_count(self) -> int:
        return len(self._cases)

    def __len__(self) -> int:
        return self.case_count

    def __getitem__(self, index: int) -> bytes:
        return self._cases[index]

    def __call__(self, index: int) -> bytes:
        if index < 0:
            raise ValueError("index must be non-negative")
        try:
            return self._cases[index]
        except IndexError as exc:
            raise IndexError("mutation schedule exhausted") from exc
