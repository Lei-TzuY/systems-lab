import pytest

from systems_conformance.comparator import ComparisonResult
from systems_conformance.fuzz import run_failure_discovery_campaign


def _comparison(classification: str, mismatches: tuple[str, ...]) -> ComparisonResult:
    if classification == "match":
        return ComparisonResult(equivalent=True, classification="match", mismatches=())
    return ComparisonResult(
        equivalent=False,
        classification=classification,  # type: ignore[arg-type]
        mismatches=mismatches,
        candidate_infrastructure_error=(
            "spawn failed" if classification == "infrastructure_failure" else None
        ),
    )


def test_discovery_keeps_first_witness_per_stable_signature() -> None:
    results = (
        _comparison("match", ()),
        _comparison("product_mismatch", ("stdout",)),
        _comparison("product_mismatch", ("stdout",)),
        _comparison("product_mismatch", ("exit_code",)),
    )

    campaign = run_failure_discovery_campaign(
        cases=lambda index: index,
        evaluate=lambda case: results[case],
        max_evaluations=len(results),
        max_unique_failures=4,
    )

    assert campaign.evaluations == 4
    assert campaign.exhausted_budget is True
    assert campaign.reached_failure_limit is False
    assert [failure.case for failure in campaign.failures] == [1, 3]
    assert [failure.evaluation_index for failure in campaign.failures] == [1, 3]
    assert [failure.signature.dimensions for failure in campaign.failures] == [
        ("stdout",),
        ("exit_code",),
    ]


def test_discovery_preserves_infrastructure_failure_identity() -> None:
    results = (
        _comparison("infrastructure_failure", ("candidate.infrastructure_error",)),
        _comparison("product_mismatch", ("stdout",)),
    )

    campaign = run_failure_discovery_campaign(
        cases=lambda index: index,
        evaluate=lambda case: results[case],
        max_evaluations=2,
        max_unique_failures=2,
    )

    assert campaign.reached_failure_limit is True
    assert campaign.exhausted_budget is False
    assert [failure.signature.kind for failure in campaign.failures] == [
        "infrastructure_failure",
        "product_mismatch",
    ]


def test_discovery_stops_at_unique_failure_limit() -> None:
    campaign = run_failure_discovery_campaign(
        cases=lambda index: index,
        evaluate=lambda case: _comparison("product_mismatch", (str(case),)),
        max_evaluations=10,
        max_unique_failures=2,
    )

    assert campaign.evaluations == 2
    assert campaign.reached_failure_limit is True
    assert campaign.exhausted_budget is False
    assert [failure.case for failure in campaign.failures] == [0, 1]


def test_discovery_rejects_non_positive_limits() -> None:
    with pytest.raises(ValueError, match="max_evaluations must be positive"):
        run_failure_discovery_campaign(
            cases=lambda index: index,
            evaluate=lambda case: _comparison("match", ()),
            max_evaluations=0,
        )

    with pytest.raises(ValueError, match="max_unique_failures must be positive"):
        run_failure_discovery_campaign(
            cases=lambda index: index,
            evaluate=lambda case: _comparison("match", ()),
            max_unique_failures=0,
        )


def test_discovery_rejects_inconsistent_comparison_records() -> None:
    bad = ComparisonResult(
        equivalent=True,
        classification="product_mismatch",
        mismatches=("stdout",),
    )

    with pytest.raises(ValueError, match="inconsistent comparison result"):
        run_failure_discovery_campaign(
            cases=lambda index: index,
            evaluate=lambda case: bad,
            max_evaluations=1,
        )
