from systems_conformance.byte_reducer import hierarchical_byte_deletions


def test_hierarchical_deletions_are_deterministic_strict_and_unique() -> None:
    value = b"abcdefgh"

    first = list(hierarchical_byte_deletions(value))
    second = list(hierarchical_byte_deletions(value))

    assert first == second
    assert first[:2] == [b"", b"efgh"]
    assert len(first) == len(set(first))
    assert all(len(candidate) < len(value) for candidate in first)
    assert all(candidate != value for candidate in first)


def test_hierarchical_deletions_reach_single_byte_granularity() -> None:
    candidates = list(hierarchical_byte_deletions(b"abc"))

    assert b"bc" in candidates
    assert b"ac" in candidates
    assert b"ab" in candidates


def test_empty_input_has_no_candidates() -> None:
    assert list(hierarchical_byte_deletions(b"")) == []
