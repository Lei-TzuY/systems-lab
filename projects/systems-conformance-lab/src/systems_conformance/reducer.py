from __future__ import annotations

from collections.abc import Callable, Iterable
from dataclasses import dataclass
from typing import Generic, TypeVar

CaseT = TypeVar("CaseT")


@dataclass(frozen=True, slots=True)
class ReductionResult(Generic[CaseT]):
    """Outcome of one deterministic greedy reduction run."""

    original: CaseT
    reduced: CaseT
    evaluations: int
    accepted_steps: int
    exhausted_budget: bool


def reduce_case(
    initial: CaseT,
    *,
    candidates: Callable[[CaseT], Iterable[CaseT]],
    preserves_failure: Callable[[CaseT], bool],
    measure: Callable[[CaseT], int],
    max_evaluations: int = 1_000,
) -> ReductionResult[CaseT]:
    """Greedily reduce a failing case while preserving its failure class.

    Candidate order is significant and therefore defines deterministic
    first-improvement behavior. A candidate is only evaluated when its measure
    is strictly smaller than the current case, which prevents cycles and makes
    progress explicit. The initial case must reproduce the target failure.
    Predicate exceptions are intentionally not swallowed so harness failures
    cannot be mistaken for product-level non-reproduction.
    """

    if max_evaluations <= 0:
        raise ValueError("max_evaluations must be positive")

    initial_measure = measure(initial)
    if initial_measure < 0:
        raise ValueError("measure must be non-negative")

    evaluations = 1
    if not preserves_failure(initial):
        raise ValueError("initial case does not preserve the target failure")

    current = initial
    current_measure = initial_measure
    accepted_steps = 0

    while evaluations < max_evaluations:
        accepted = False
        for candidate in candidates(current):
            candidate_measure = measure(candidate)
            if candidate_measure < 0:
                raise ValueError("measure must be non-negative")
            if candidate_measure >= current_measure:
                continue
            if evaluations >= max_evaluations:
                return ReductionResult(
                    original=initial,
                    reduced=current,
                    evaluations=evaluations,
                    accepted_steps=accepted_steps,
                    exhausted_budget=True,
                )

            evaluations += 1
            if preserves_failure(candidate):
                current = candidate
                current_measure = candidate_measure
                accepted_steps += 1
                accepted = True
                break

        if not accepted:
            return ReductionResult(
                original=initial,
                reduced=current,
                evaluations=evaluations,
                accepted_steps=accepted_steps,
                exhausted_budget=False,
            )

    return ReductionResult(
        original=initial,
        reduced=current,
        evaluations=evaluations,
        accepted_steps=accepted_steps,
        exhausted_budget=True,
    )
