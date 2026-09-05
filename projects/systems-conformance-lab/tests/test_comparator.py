from __future__ import annotations

import json

from systems_conformance.comparator import compare_results
from systems_conformance.model import ExecutionResult, StreamCapture


def _capture(text: str = "", *, total_bytes: int | None = None, truncated: bool = False) -> StreamCapture:
    encoded = text.encode("utf-8")
    return StreamCapture(
        text=text,
        total_bytes=len(encoded) if total_bytes is None else total_bytes,
        truncated=truncated,
    )


def _result(
    *,
    timed_out: bool = False,
    exit_code: int | None = 0,
    signal: int | None = None,
    stdout: StreamCapture | None = None,
    stderr: StreamCapture | None = None,
    infrastructure_error: str | None = None,
) -> ExecutionResult:
    return ExecutionResult(
        argv=("tool",),
        duration_ms=1,
        timed_out=timed_out,
        exit_code=exit_code,
        signal=signal,
        stdout=stdout or _capture(),
        stderr=stderr or _capture(),
        infrastructure_error=infrastructure_error,
    )


def test_identical_results_match() -> None:
    candidate = _result(stdout=_capture("ok\n"))
    comparison = compare_results(candidate, candidate)

    assert comparison.equivalent is True
    assert comparison.classification == "match"
    assert comparison.mismatches == ()
    json.dumps(comparison.to_dict())


def test_infrastructure_failure_takes_precedence() -> None:
    candidate = _result(infrastructure_error="spawn failed", exit_code=None)
    oracle = _result(stdout=_capture("different"))

    comparison = compare_results(candidate, oracle)

    assert comparison.equivalent is False
    assert comparison.classification == "infrastructure_failure"
    assert comparison.mismatches == ("candidate.infrastructure_error",)
    assert comparison.candidate_infrastructure_error == "spawn failed"


def test_timeout_difference_is_product_mismatch() -> None:
    candidate = _result(timed_out=True, exit_code=None)
    oracle = _result()

    comparison = compare_results(candidate, oracle)

    assert comparison.classification == "product_mismatch"
    assert comparison.mismatches == ("timed_out",)


def test_both_timeouts_ignore_partial_output() -> None:
    candidate = _result(timed_out=True, exit_code=None, stdout=_capture("candidate partial"))
    oracle = _result(timed_out=True, exit_code=None, stdout=_capture("oracle partial"))

    comparison = compare_results(candidate, oracle)

    assert comparison.equivalent is True
    assert comparison.classification == "match"
    assert comparison.mismatches == ()


def test_reports_all_product_mismatches_in_stable_order() -> None:
    candidate = _result(
        exit_code=2,
        signal=9,
        stdout=_capture("left"),
        stderr=_capture("candidate error"),
    )
    oracle = _result(
        exit_code=3,
        signal=15,
        stdout=_capture("right"),
        stderr=_capture("oracle error"),
    )

    comparison = compare_results(candidate, oracle)

    assert comparison.classification == "product_mismatch"
    assert comparison.mismatches == ("exit_code", "signal", "stdout", "stderr")


def test_truncated_stream_metadata_prevents_false_match() -> None:
    candidate = _result(stdout=_capture("same-prefix", total_bytes=100, truncated=True))
    oracle = _result(stdout=_capture("same-prefix", total_bytes=200, truncated=True))

    comparison = compare_results(candidate, oracle)

    assert comparison.classification == "product_mismatch"
    assert comparison.mismatches == ("stdout",)
