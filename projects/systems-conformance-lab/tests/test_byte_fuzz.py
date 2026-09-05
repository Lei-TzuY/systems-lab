import pytest

from systems_conformance import DeterministicByteMutations


def test_schedule_is_deterministic_and_bit_granular() -> None:
    cases = DeterministicByteMutations((b"\x00",))

    assert len(cases) == 9
    assert [cases(index) for index in range(len(cases))] == [
        b"\x00",
        b"\x01",
        b"\x02",
        b"\x04",
        b"\x08",
        b"\x10",
        b"\x20",
        b"\x40",
        b"\x80",
    ]


def test_schedule_deduplicates_identical_seeds_and_mutations() -> None:
    cases = DeterministicByteMutations((b"A", b"A"))

    assert len(cases) == 9
    assert len({cases(index) for index in range(len(cases))}) == len(cases)


def test_schedule_enforces_case_size_boundary() -> None:
    with pytest.raises(ValueError, match="seed exceeds max_case_bytes"):
        DeterministicByteMutations((b"AB",), max_case_bytes=1)


def test_schedule_rejects_invalid_access_and_input() -> None:
    cases = DeterministicByteMutations((b"A",))

    with pytest.raises(ValueError, match="index must be non-negative"):
        cases(-1)
    with pytest.raises(IndexError, match="mutation schedule exhausted"):
        cases(len(cases))
    with pytest.raises(ValueError, match="seeds must be non-empty"):
        DeterministicByteMutations(())
