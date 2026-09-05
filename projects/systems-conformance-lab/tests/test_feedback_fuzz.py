import pytest

from systems_conformance import ComparisonResult, run_feedback_guided_campaign

MATCH = ComparisonResult(equivalent=True, classification="match", mismatches=())
MISMATCH = ComparisonResult(
    equivalent=False, classification="product_mismatch", mismatches=("stdout",)
)
INFRA_FAILURE = ComparisonResult(
    equivalent=False,
    classification="infrastructure_failure",
    mismatches=("candidate.infrastructure_error",),
    candidate_infrastructure_error="SpawnError: denied",
)


def test_admits_only_first_witness_for_new_features() -> None:
    def evaluate(case: bytes):
        return MATCH, {case[:1], case[-1:]}

    result = run_feedback_guided_campaign(
        seeds=(b"aa",),
        mutate=lambda case, index: bytes((case[0], ord("b") + index)),
        evaluate=evaluate,
        mutations_per_case=2,
        max_evaluations=5,
    )

    assert [entry.case for entry in result.corpus] == [b"aa", b"ab", b"ac"]
    assert result.features == frozenset({b"a", b"b", b"c"})
    assert result.failures == ()
    assert result.evaluations == 5
    assert result.exhausted_budget


def test_retains_first_witness_for_each_stable_failure() -> None:
    comparisons = {0: MATCH, 1: MISMATCH, 2: MISMATCH, 3: INFRA_FAILURE}
    result = run_feedback_guided_campaign(
        seeds=(0, 1, 2, 3),
        mutate=lambda case, index: case,
        evaluate=lambda case: (comparisons[case], {"shared"}),
        max_evaluations=4,
    )

    assert [failure.case for failure in result.failures] == [1, 3]
    assert [failure.evaluation_index for failure in result.failures] == [1, 3]
    assert [failure.comparison.classification for failure in result.failures] == [
        "product_mismatch",
        "infrastructure_failure",
    ]
    assert len({failure.signature for failure in result.failures}) == 2


def test_stops_at_unique_failure_limit() -> None:
    result = run_feedback_guided_campaign(
        seeds=(0, 1),
        mutate=lambda case, index: case,
        evaluate=lambda case: (
            MISMATCH if case == 0 else INFRA_FAILURE,
            {case},
        ),
        max_unique_failures=1,
    )

    assert result.evaluations == 1
    assert [failure.case for failure in result.failures] == [0]
    assert result.reached_failure_limit
    assert not result.exhausted_budget


def test_stops_at_corpus_limit() -> None:
    result = run_feedback_guided_campaign(
        seeds=(0,),
        mutate=lambda case, index: case + index + 1,
        evaluate=lambda case: (MATCH, {case}),
        max_corpus_entries=2,
    )

    assert [entry.case for entry in result.corpus] == [0, 1]
    assert result.reached_corpus_limit
    assert not result.reached_failure_limit
    assert not result.exhausted_budget
    assert result.evaluations == 2


@pytest.mark.parametrize(
    ("kwargs", "message"),
    [
        ({"seeds": ()}, "seeds"),
        ({"mutations_per_case": 0}, "mutations_per_case"),
        ({"max_evaluations": 0}, "max_evaluations"),
        ({"max_corpus_entries": 0}, "max_corpus_entries"),
        ({"max_unique_failures": 0}, "max_unique_failures"),
    ],
)
def test_rejects_invalid_bounds(kwargs, message) -> None:
    defaults = {
        "seeds": (b"x",),
        "mutate": lambda case, index: case,
        "evaluate": lambda case: (MATCH, {case}),
    }
    defaults.update(kwargs)
    with pytest.raises(ValueError, match=message):
        run_feedback_guided_campaign(**defaults)


def test_rejects_inconsistent_comparison() -> None:
    bad = ComparisonResult(equivalent=False, classification="match", mismatches=())
    with pytest.raises(ValueError, match="inconsistent"):
        run_feedback_guided_campaign(
            seeds=(b"x",),
            mutate=lambda case, index: case,
            evaluate=lambda case: (bad, {"feature"}),
        )
