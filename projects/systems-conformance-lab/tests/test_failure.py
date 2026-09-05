from __future__ import annotations

import json

from systems_conformance.comparator import ComparisonResult
from systems_conformance.failure import failure_signature


def test_match_has_no_failure_signature() -> None:
    comparison = ComparisonResult(
        equivalent=True,
        classification="match",
        mismatches=(),
    )

    assert failure_signature(comparison) is None


def test_product_signature_uses_stable_mismatch_dimensions() -> None:
    comparison = ComparisonResult(
        equivalent=False,
        classification="product_mismatch",
        mismatches=("exit_code", "stdout"),
    )

    signature = failure_signature(comparison)

    assert signature is not None
    assert signature.kind == "product_mismatch"
    assert signature.dimensions == ("exit_code", "stdout")
    json.dumps(signature.to_dict())


def test_infrastructure_signature_excludes_volatile_error_text() -> None:
    first = ComparisonResult(
        equivalent=False,
        classification="infrastructure_failure",
        mismatches=("candidate.infrastructure_error",),
        candidate_infrastructure_error="[Errno 2] executable not found",
    )
    second = ComparisonResult(
        equivalent=False,
        classification="infrastructure_failure",
        mismatches=("candidate.infrastructure_error",),
        candidate_infrastructure_error="CreateProcess failed with platform-specific text",
    )

    assert failure_signature(first) == failure_signature(second)


def test_failure_signature_preserves_side_identity() -> None:
    candidate_failure = ComparisonResult(
        equivalent=False,
        classification="infrastructure_failure",
        mismatches=("candidate.infrastructure_error",),
        candidate_infrastructure_error="spawn failed",
    )
    oracle_failure = ComparisonResult(
        equivalent=False,
        classification="infrastructure_failure",
        mismatches=("oracle.infrastructure_error",),
        oracle_infrastructure_error="spawn failed",
    )

    assert failure_signature(candidate_failure) != failure_signature(oracle_failure)
