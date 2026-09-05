from collections.abc import Callable
from dataclasses import dataclass
from typing import Generic, Literal, TypeVar

from .comparator import ComparisonResult
from .failure import FailureSignature, failure_signature

CaseT = TypeVar("CaseT")
FuzzClassification = Literal["match", "product_mismatch", "infrastructure_failure"]


@dataclass(frozen=True, slots=True)
class FuzzCampaignResult(Generic[CaseT]):
    """Outcome of one deterministic, bounded fuzz campaign."""

    evaluations: int
    classification: FuzzClassification
    failing_case: CaseT | None
    comparison: ComparisonResult | None
    exhausted_budget: bool


@dataclass(frozen=True, slots=True)
class FuzzFailure(Generic[CaseT]):
    """First observed case for one stable failure signature."""

    evaluation_index: int
    case: CaseT
    comparison: ComparisonResult
    signature: FailureSignature


@dataclass(frozen=True, slots=True)
class FuzzDiscoveryResult(Generic[CaseT]):
    """Outcome of deterministic discovery across distinct failure signatures."""

    evaluations: int
    failures: tuple[FuzzFailure[CaseT], ...]
    exhausted_budget: bool
    reached_failure_limit: bool


def run_fuzz_campaign(
    *,
    cases: Callable[[int], CaseT],
    evaluate: Callable[[CaseT], ComparisonResult],
    max_evaluations: int = 1_000,
) -> FuzzCampaignResult[CaseT]:
    """Evaluate deterministic cases until failure or budget exhaustion.

    The case source receives a monotonically increasing zero-based index rather
    than shared mutable randomness. This keeps scheduling deterministic while
    allowing adapters to derive seeded or corpus-based cases however they need.

    Product mismatches and infrastructure failures are terminal but remain
    explicitly distinct. Exceptions from case generation or evaluation are not
    swallowed: a broken harness must not be converted into a product result.
    """

    if max_evaluations <= 0:
        raise ValueError("max_evaluations must be positive")

    for index in range(max_evaluations):
        case = cases(index)
        comparison = evaluate(case)
        _validate_comparison(comparison)

        if comparison.classification == "match":
            continue

        return FuzzCampaignResult(
            evaluations=index + 1,
            classification=comparison.classification,
            failing_case=case,
            comparison=comparison,
            exhausted_budget=False,
        )

    return FuzzCampaignResult(
        evaluations=max_evaluations,
        classification="match",
        failing_case=None,
        comparison=None,
        exhausted_budget=True,
    )


def run_failure_discovery_campaign(
    *,
    cases: Callable[[int], CaseT],
    evaluate: Callable[[CaseT], ComparisonResult],
    max_evaluations: int = 1_000,
    max_unique_failures: int = 32,
) -> FuzzDiscoveryResult[CaseT]:
    """Discover first witnesses for distinct stable failure signatures.

    Unlike ``run_fuzz_campaign``, this campaign continues after a failure so a
    bounded deterministic schedule can surface multiple independent failure
    classes. Duplicate signatures are ignored after their first witness. The
    campaign stops only when its evaluation budget is exhausted or the unique
    failure limit is reached.
    """

    if max_evaluations <= 0:
        raise ValueError("max_evaluations must be positive")
    if max_unique_failures <= 0:
        raise ValueError("max_unique_failures must be positive")

    failures: list[FuzzFailure[CaseT]] = []
    seen: set[FailureSignature] = set()

    for index in range(max_evaluations):
        case = cases(index)
        comparison = evaluate(case)
        _validate_comparison(comparison)
        signature = failure_signature(comparison)

        if signature is None or signature in seen:
            continue

        seen.add(signature)
        failures.append(
            FuzzFailure(
                evaluation_index=index,
                case=case,
                comparison=comparison,
                signature=signature,
            )
        )
        if len(failures) >= max_unique_failures:
            return FuzzDiscoveryResult(
                evaluations=index + 1,
                failures=tuple(failures),
                exhausted_budget=False,
                reached_failure_limit=True,
            )

    return FuzzDiscoveryResult(
        evaluations=max_evaluations,
        failures=tuple(failures),
        exhausted_budget=True,
        reached_failure_limit=False,
    )


def _validate_comparison(comparison: ComparisonResult) -> None:
    is_match = comparison.classification == "match"
    if comparison.equivalent != is_match:
        raise ValueError("inconsistent comparison result")
    if is_match and comparison.mismatches:
        raise ValueError("inconsistent comparison result")
    if not is_match and not comparison.mismatches:
        raise ValueError("inconsistent comparison result")
