from __future__ import annotations

from dataclasses import asdict, dataclass
from typing import Any, Literal

from .model import ExecutionResult, StreamCapture

COMPARISON_SCHEMA_VERSION = "systems-conformance.comparison.v1"

ComparisonClassification = Literal[
    "match",
    "product_mismatch",
    "infrastructure_failure",
]


@dataclass(frozen=True, slots=True)
class ComparisonResult:
    """Structured candidate/oracle comparison result."""

    equivalent: bool
    classification: ComparisonClassification
    mismatches: tuple[str, ...]
    candidate_infrastructure_error: str | None = None
    oracle_infrastructure_error: str | None = None
    schema_version: str = COMPARISON_SCHEMA_VERSION

    def to_dict(self) -> dict[str, Any]:
        """Return a JSON-serializable comparison record."""
        return asdict(self)


def compare_results(candidate: ExecutionResult, oracle: ExecutionResult) -> ComparisonResult:
    """Compare candidate and oracle execution records deterministically.

    Infrastructure failures take precedence over product-level comparison so a
    broken harness or spawn failure is never reported as a target mismatch.
    When both executions time out, partial output is intentionally ignored:
    timeout is the observable outcome and partial streams are often timing
    dependent.
    """

    infrastructure_mismatches: list[str] = []
    if candidate.infrastructure_error is not None:
        infrastructure_mismatches.append("candidate.infrastructure_error")
    if oracle.infrastructure_error is not None:
        infrastructure_mismatches.append("oracle.infrastructure_error")

    if infrastructure_mismatches:
        return ComparisonResult(
            equivalent=False,
            classification="infrastructure_failure",
            mismatches=tuple(infrastructure_mismatches),
            candidate_infrastructure_error=candidate.infrastructure_error,
            oracle_infrastructure_error=oracle.infrastructure_error,
        )

    if candidate.timed_out != oracle.timed_out:
        return ComparisonResult(
            equivalent=False,
            classification="product_mismatch",
            mismatches=("timed_out",),
        )

    if candidate.timed_out and oracle.timed_out:
        return ComparisonResult(
            equivalent=True,
            classification="match",
            mismatches=(),
        )

    mismatches: list[str] = []
    if candidate.exit_code != oracle.exit_code:
        mismatches.append("exit_code")
    if candidate.signal != oracle.signal:
        mismatches.append("signal")
    if not _same_capture(candidate.stdout, oracle.stdout):
        mismatches.append("stdout")
    if not _same_capture(candidate.stderr, oracle.stderr):
        mismatches.append("stderr")

    if mismatches:
        return ComparisonResult(
            equivalent=False,
            classification="product_mismatch",
            mismatches=tuple(mismatches),
        )

    return ComparisonResult(
        equivalent=True,
        classification="match",
        mismatches=(),
    )


def _same_capture(left: StreamCapture, right: StreamCapture) -> bool:
    return (
        left.text == right.text
        and left.total_bytes == right.total_bytes
        and left.truncated == right.truncated
    )
