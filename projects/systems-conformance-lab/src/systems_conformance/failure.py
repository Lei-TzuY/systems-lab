from __future__ import annotations

from dataclasses import asdict, dataclass
from typing import Any, Literal

from .comparator import ComparisonResult

FAILURE_SIGNATURE_SCHEMA_VERSION = "systems-conformance.failure-signature.v1"

FailureKind = Literal["product_mismatch", "infrastructure_failure"]


@dataclass(frozen=True, slots=True)
class FailureSignature:
    """Stable machine-readable identity for one comparison failure class.

    The signature intentionally excludes volatile details such as captured
    output, durations, and infrastructure exception messages. Reducers and
    repro tooling can therefore decide whether a transformed test case still
    demonstrates the same failure without relying on diagnostic text.
    """

    kind: FailureKind
    dimensions: tuple[str, ...]
    schema_version: str = FAILURE_SIGNATURE_SCHEMA_VERSION

    def to_dict(self) -> dict[str, Any]:
        """Return a JSON-serializable signature record."""
        return asdict(self)


def failure_signature(comparison: ComparisonResult) -> FailureSignature | None:
    """Return a stable signature for a failing comparison.

    Matching comparisons have no failure signature. Product mismatches retain
    the comparator's deterministic mismatch dimension ordering. Infrastructure
    failures retain only which side failed, not the unstable error message.
    """

    if comparison.equivalent:
        return None

    if comparison.classification == "product_mismatch":
        return FailureSignature(
            kind="product_mismatch",
            dimensions=comparison.mismatches,
        )

    if comparison.classification == "infrastructure_failure":
        return FailureSignature(
            kind="infrastructure_failure",
            dimensions=comparison.mismatches,
        )

    raise ValueError(f"unsupported failing classification: {comparison.classification!r}")
