from collections.abc import Callable, Hashable, Iterable, Sequence
from dataclasses import dataclass
from typing import Generic, TypeVar

from .comparator import ComparisonResult
from .failure import FailureSignature, failure_signature
from .fuzz import FuzzFailure

CaseT = TypeVar("CaseT")
FeatureT = TypeVar("FeatureT", bound=Hashable)


@dataclass(frozen=True, slots=True)
class FeedbackCorpusEntry(Generic[CaseT, FeatureT]):
    case: CaseT
    new_features: frozenset[FeatureT]


@dataclass(frozen=True, slots=True)
class FeedbackCampaignResult(Generic[CaseT, FeatureT]):
    evaluations: int
    corpus: tuple[FeedbackCorpusEntry[CaseT, FeatureT], ...]
    features: frozenset[FeatureT]
    failures: tuple[FuzzFailure[CaseT], ...]
    exhausted_budget: bool
    reached_corpus_limit: bool
    reached_failure_limit: bool


def run_feedback_guided_campaign(
    *,
    seeds: Sequence[CaseT],
    mutate: Callable[[CaseT, int], CaseT],
    evaluate: Callable[[CaseT], tuple[ComparisonResult, Iterable[FeatureT]]],
    mutations_per_case: int = 8,
    max_evaluations: int = 1_000,
    max_corpus_entries: int = 256,
    max_unique_failures: int = 32,
) -> FeedbackCampaignResult[CaseT, FeatureT]:
    """Grow a deterministic corpus while retaining stable failure witnesses.

    Seeds are evaluated in order. Each admitted entry is then visited in FIFO
    order and receives exactly ``mutations_per_case`` indexed mutations. A case
    is admitted only when it contributes at least one previously unseen feature.
    Independently, the first witness for each stable failure signature is retained
    so expensive feedback executions do not discard product or infrastructure
    failures. Evaluation, corpus, and unique-failure growth are all strictly bounded.
    """

    if not seeds:
        raise ValueError("seeds must not be empty")
    if mutations_per_case <= 0:
        raise ValueError("mutations_per_case must be positive")
    if max_evaluations <= 0:
        raise ValueError("max_evaluations must be positive")
    if max_corpus_entries <= 0:
        raise ValueError("max_corpus_entries must be positive")
    if max_unique_failures <= 0:
        raise ValueError("max_unique_failures must be positive")

    corpus: list[FeedbackCorpusEntry[CaseT, FeatureT]] = []
    seen_features: set[FeatureT] = set()
    failures: list[FuzzFailure[CaseT]] = []
    seen_failures: set[FailureSignature] = set()
    evaluations = 0

    def consider(case: CaseT) -> tuple[bool, bool]:
        nonlocal evaluations
        comparison, raw_features = evaluate(case)
        _validate_comparison(comparison)
        evaluation_index = evaluations
        evaluations += 1

        signature = failure_signature(comparison)
        if signature is not None and signature not in seen_failures:
            seen_failures.add(signature)
            failures.append(
                FuzzFailure(
                    evaluation_index=evaluation_index,
                    case=case,
                    comparison=comparison,
                    signature=signature,
                )
            )

        features = frozenset(raw_features)
        new_features = features.difference(seen_features)
        if new_features:
            seen_features.update(new_features)
            corpus.append(FeedbackCorpusEntry(case=case, new_features=frozenset(new_features)))

        return len(corpus) >= max_corpus_entries, len(failures) >= max_unique_failures

    for seed in seeds:
        if evaluations >= max_evaluations:
            break
        corpus_limit, failure_limit = consider(seed)
        if corpus_limit or failure_limit:
            return _result(
                evaluations,
                corpus,
                seen_features,
                failures,
                False,
                corpus_limit,
                failure_limit,
            )

    parent_index = 0
    while parent_index < len(corpus) and evaluations < max_evaluations:
        parent = corpus[parent_index].case
        parent_index += 1
        for mutation_index in range(mutations_per_case):
            if evaluations >= max_evaluations:
                break
            corpus_limit, failure_limit = consider(mutate(parent, mutation_index))
            if corpus_limit or failure_limit:
                return _result(
                    evaluations,
                    corpus,
                    seen_features,
                    failures,
                    False,
                    corpus_limit,
                    failure_limit,
                )

    return _result(
        evaluations,
        corpus,
        seen_features,
        failures,
        evaluations >= max_evaluations,
        False,
        False,
    )


def _result(
    evaluations: int,
    corpus: list[FeedbackCorpusEntry[CaseT, FeatureT]],
    features: set[FeatureT],
    failures: list[FuzzFailure[CaseT]],
    exhausted_budget: bool,
    reached_corpus_limit: bool,
    reached_failure_limit: bool,
) -> FeedbackCampaignResult[CaseT, FeatureT]:
    return FeedbackCampaignResult(
        evaluations=evaluations,
        corpus=tuple(corpus),
        features=frozenset(features),
        failures=tuple(failures),
        exhausted_budget=exhausted_budget,
        reached_corpus_limit=reached_corpus_limit,
        reached_failure_limit=reached_failure_limit,
    )


def _validate_comparison(comparison: ComparisonResult) -> None:
    is_match = comparison.classification == "match"
    inconsistent = (
        comparison.equivalent != is_match
        or (is_match and bool(comparison.mismatches))
        or (not is_match and not comparison.mismatches)
    )
    if inconsistent:
        raise ValueError("inconsistent comparison result")
