from __future__ import annotations

import pytest

from systems_conformance import reduce_case


def test_reducer_accepts_first_strictly_smaller_failure_preserving_candidate() -> None:
    seen: list[str] = []

    def candidates(value: str) -> list[str]:
        if value == "abcdef":
            return ["abcdef", "abcde", "abc", "ab"]
        if value == "abc":
            return ["ab", "a"]
        return []

    def preserves(value: str) -> bool:
        seen.append(value)
        return value in {"abcdef", "abc"}

    result = reduce_case(
        "abcdef",
        candidates=candidates,
        preserves_failure=preserves,
        measure=len,
    )

    assert result.original == "abcdef"
    assert result.reduced == "abc"
    assert result.accepted_steps == 1
    assert result.evaluations == 5
    assert result.exhausted_budget is False
    assert seen == ["abcdef", "abcde", "abc", "ab", "a"]


def test_reducer_requires_initial_case_to_reproduce_failure() -> None:
    with pytest.raises(ValueError, match="initial case"):
        reduce_case(
            "ok",
            candidates=lambda _: [],
            preserves_failure=lambda _: False,
            measure=len,
        )


def test_reducer_skips_non_progressing_candidates_without_evaluation() -> None:
    evaluated: list[str] = []

    result = reduce_case(
        "abc",
        candidates=lambda _: ["abc", "abcd", "ab"],
        preserves_failure=lambda value: evaluated.append(value) is None and value != "ab",
        measure=len,
    )

    assert result.reduced == "abc"
    assert result.evaluations == 2
    assert evaluated == ["abc", "ab"]


def test_reducer_stops_at_evaluation_budget() -> None:
    result = reduce_case(
        "abcd",
        candidates=lambda value: [value[:-1]],
        preserves_failure=lambda _: True,
        measure=len,
        max_evaluations=3,
    )

    assert result.reduced == "ab"
    assert result.evaluations == 3
    assert result.accepted_steps == 2
    assert result.exhausted_budget is True


def test_reducer_rejects_invalid_budget_and_negative_measure() -> None:
    with pytest.raises(ValueError, match="max_evaluations"):
        reduce_case(
            "x",
            candidates=lambda _: [],
            preserves_failure=lambda _: True,
            measure=len,
            max_evaluations=0,
        )

    with pytest.raises(ValueError, match="non-negative"):
        reduce_case(
            "x",
            candidates=lambda _: [],
            preserves_failure=lambda _: True,
            measure=lambda _: -1,
        )


def test_reducer_does_not_hide_predicate_exceptions() -> None:
    def preserves(value: str) -> bool:
        if value == "ab":
            raise RuntimeError("harness failed")
        return True

    with pytest.raises(RuntimeError, match="harness failed"):
        reduce_case(
            "abc",
            candidates=lambda _: ["ab"],
            preserves_failure=preserves,
            measure=len,
        )
