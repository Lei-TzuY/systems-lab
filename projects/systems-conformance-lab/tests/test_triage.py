from __future__ import annotations

import json
import sys

import pytest

from systems_conformance import (
    CommandTarget,
    DifferentialHarness,
    FailureSignature,
    FuzzFailure,
    hierarchical_byte_deletions,
    reduce_failure_to_repro,
    run_failure_discovery_campaign,
)
from systems_conformance.comparator import ComparisonResult

ECHO_SCRIPT = "import sys; data=sys.stdin.buffer.read(); sys.stdout.buffer.write(data)"
BUGGY_SCRIPT = (
    "import sys; data=sys.stdin.buffer.read(); "
    "sys.stdout.buffer.write(data.replace(b'BUG', b'BAD') if b'BUG' in data else data)"
)


def target(script: str) -> CommandTarget:
    return CommandTarget((sys.executable, "-c", script))


def test_rejects_inconsistent_captured_signature_before_execution(tmp_path) -> None:
    comparison = ComparisonResult(
        equivalent=False,
        classification="product_mismatch",
        mismatches=("stdout",),
    )
    failure = FuzzFailure(
        evaluation_index=3,
        case=b"BUG",
        comparison=comparison,
        signature=FailureSignature(kind="product_mismatch", dimensions=("exit_code",)),
    )
    harness = DifferentialHarness(candidate=target(BUGGY_SCRIPT), oracle=target(ECHO_SCRIPT))

    with pytest.raises(ValueError, match="inconsistent stable signature"):
        reduce_failure_to_repro(
            failure,
            harness=harness,
            destination=tmp_path / "repro",
            candidates=hierarchical_byte_deletions,
        )

    assert not (tmp_path / "repro").exists()


def test_real_fuzz_witness_reduces_and_publishes_same_failure(tmp_path) -> None:
    harness = DifferentialHarness(candidate=target(BUGGY_SCRIPT), oracle=target(ECHO_SCRIPT))
    corpus = (b"ordinary", b"prefix BUG suffix")
    discovery = run_failure_discovery_campaign(
        cases=corpus.__getitem__,
        evaluate=harness.compare,
        max_evaluations=len(corpus),
        max_unique_failures=4,
    )

    assert len(discovery.failures) == 1
    failure = discovery.failures[0]

    result = reduce_failure_to_repro(
        failure,
        harness=harness,
        destination=tmp_path / "repro",
        candidates=hierarchical_byte_deletions,
        max_evaluations=100,
        metadata={"source": "fuzz-witness-triage"},
    )

    assert result.failure is failure
    assert result.reduction.reduced == b"BUG"
    assert result.reduction.accepted_steps > 0
    assert result.repro.input_path.read_bytes() == b"BUG"

    manifest = json.loads(result.repro.manifest_path.read_text(encoding="utf-8"))
    assert manifest["failure_signature"]["kind"] == failure.signature.kind
    assert tuple(manifest["failure_signature"]["dimensions"]) == failure.signature.dimensions
    assert manifest["comparison"]["classification"] == "product_mismatch"
    assert manifest["metadata"] == {"source": "fuzz-witness-triage"}

    replay = harness.replay_repro(result.repro.path)
    assert replay.reproduced is True
    assert replay.run.signature == failure.signature
