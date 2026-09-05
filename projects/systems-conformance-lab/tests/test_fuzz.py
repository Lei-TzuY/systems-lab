import pytest

from systems_conformance.comparator import ComparisonResult
from systems_conformance.fuzz import run_fuzz_campaign


def _comparison(classification: str) -> ComparisonResult:
    if classification == "match":
        return ComparisonResult(equivalent=True, classification="match", mismatches=())
    if classification == "product_mismatch":
        return ComparisonResult(
            equivalent=False,
            classification="product_mismatch",
            mismatches=("stdout",),
        )
    if classification == "infrastructure_failure":
        return ComparisonResult(
            equivalent=False,
            classification="infrastructure_failure",
            mismatches=("candidate.infrastructure_error",),
            candidate_infrastructure_error="spawn failed",
        )
    raise AssertionError(classification)


def test_campaign_stops_at_first_product_failure() -> None:
    seen: list[int] = []

    def evaluate(case: int) -> ComparisonResult:
        seen.append(case)
        return _comparison("product_mismatch" if case == 2 else "match")

    result = run_fuzz_campaign(
        cases=lambda index: index,
        evaluate=evaluate,
        max_evaluations=10,
    )

    assert seen == [0, 1, 2]
    assert result.evaluations == 3
    assert result.classification == "product_mismatch"
    assert result.failing_case == 2
    assert result.comparison == _comparison("product_mismatch")
    assert result.exhausted_budget is False


def test_campaign_preserves_infrastructure_failure_classification() -> None:
    result = run_fuzz_campaign(
        cases=lambda index: index,
        evaluate=lambda case: _comparison(
            "infrastructure_failure" if case == 1 else "match"
        ),
        max_evaluations=5,
    )

    assert result.evaluations == 2
    assert result.classification == "infrastructure_failure"
    assert result.failing_case == 1
    assert result.comparison == _comparison("infrastructure_failure")
    assert result.exhausted_budget is False


def test_campaign_reports_clean_budget_exhaustion() -> None:
    result = run_fuzz_campaign(
        cases=lambda index: index,
        evaluate=lambda case: _comparison("match"),
        max_evaluations=4,
    )

    assert result.evaluations == 4
    assert result.classification == "match"
    assert result.failing_case is None
    assert result.comparison is None
    assert result.exhausted_budget is True


def test_campaign_case_generation_is_index_deterministic() -> None:
    generated: list[int] = []

    def cases(index: int) -> bytes:
        generated.append(index)
        return bytes([index])

    result = run_fuzz_campaign(
        cases=cases,
        evaluate=lambda case: _comparison("match"),
        max_evaluations=3,
    )

    assert generated == [0, 1, 2]
    assert result.evaluations == 3


def test_campaign_rejects_non_positive_budget() -> None:
    with pytest.raises(ValueError, match="max_evaluations must be positive"):
        run_fuzz_campaign(
            cases=lambda index: index,
            evaluate=lambda case: _comparison("match"),
            max_evaluations=0,
        )


def test_campaign_does_not_hide_evaluator_exceptions() -> None:
    def evaluate(case: int) -> ComparisonResult:
        raise RuntimeError("harness broke")

    with pytest.raises(RuntimeError, match="harness broke"):
        run_fuzz_campaign(
            cases=lambda index: index,
            evaluate=evaluate,
            max_evaluations=1,
        )


def test_campaign_rejects_inconsistent_comparison_records() -> None:
    bad = ComparisonResult(
        equivalent=True,
        classification="product_mismatch",
        mismatches=("stdout",),
    )

    with pytest.raises(ValueError, match="inconsistent comparison result"):
        run_fuzz_campaign(
            cases=lambda index: index,
            evaluate=lambda case: bad,
            max_evaluations=1,
        )
