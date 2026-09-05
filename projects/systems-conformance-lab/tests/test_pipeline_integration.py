import json
import sys

from systems_conformance import (
    CommandTarget,
    DeterministicByteMutations,
    DifferentialHarness,
    failure_signature,
    reduce_case,
    run_failure_discovery_campaign,
    run_fuzz_campaign,
)
from systems_conformance.byte_reducer import hierarchical_byte_deletions

ECHO_SCRIPT = (
    "import sys; data = sys.stdin.buffer.read(); sys.stdout.buffer.write(data)"
)
BUGGY_SCRIPT = (
    "import sys; data = sys.stdin.buffer.read(); "
    "sys.stdout.buffer.write(data.replace(b'BUG', b'BAD') if b'BUG' in data else data)"
)
HIGH_BIT_BUGGY_SCRIPT = (
    "import sys; data = sys.stdin.buffer.read(); "
    "sys.stdout.buffer.write(b'BAD' if data == b'\\x80' else data)"
)
MULTI_BUG_SCRIPT = """
import sys

data = sys.stdin.buffer.read()
if data.startswith(b"out"):
    sys.stdout.buffer.write(b"BAD")
elif data == b"exit":
    raise SystemExit(7)
else:
    sys.stdout.buffer.write(data)
"""
WRITE_FAULT_SCRIPT = """
import sys
import tempfile

from systems_conformance import FaultSpec, FaultingBinaryWriter

data = sys.stdin.buffer.read()
with tempfile.TemporaryFile() as sink:
    writer = FaultingBinaryWriter(
        sink,
        FaultSpec(operation="write", occurrence=0, kind="short_write"),
        short_write_bytes=2,
    )
    writer.write(data)
    sink.seek(0)
    sys.stdout.buffer.write(sink.read())
"""


def target(script: str) -> CommandTarget:
    return CommandTarget((sys.executable, "-c", script))


def test_real_target_pipeline_finds_reduces_and_persists_failure(tmp_path) -> None:
    harness = DifferentialHarness(candidate=target(BUGGY_SCRIPT), oracle=target(ECHO_SCRIPT))
    corpus = (b"ordinary input", b"prefix BUG suffix")

    campaign = run_fuzz_campaign(
        cases=corpus.__getitem__,
        evaluate=harness.compare,
        max_evaluations=len(corpus),
    )

    assert campaign.evaluations == 2
    assert campaign.classification == "product_mismatch"
    assert campaign.failing_case == b"prefix BUG suffix"
    assert campaign.comparison is not None

    signature = failure_signature(campaign.comparison)
    assert signature is not None
    assert signature.dimensions == ("stdout",)

    reduction = reduce_case(
        campaign.failing_case,
        candidates=hierarchical_byte_deletions,
        preserves_failure=lambda case: harness.preserves_failure(case, signature),
        measure=len,
        max_evaluations=100,
    )

    assert reduction.reduced == b"BUG"
    assert reduction.accepted_steps > 0
    assert reduction.evaluations < 40

    bundle = harness.write_repro(
        tmp_path / "repro",
        input_bytes=reduction.reduced,
        expected_signature=signature,
        metadata={"source": "end-to-end-integration"},
    )

    assert bundle.input_path.read_bytes() == b"BUG"
    manifest = json.loads(bundle.manifest_path.read_text(encoding="utf-8"))
    assert manifest["failure_signature"]["kind"] == signature.kind
    assert tuple(manifest["failure_signature"]["dimensions"]) == signature.dimensions
    assert manifest["failure_signature"]["schema_version"] == signature.schema_version
    assert manifest["comparison"]["classification"] == "product_mismatch"
    assert manifest["metadata"] == {"source": "end-to-end-integration"}


def test_deterministic_byte_mutations_find_real_process_mismatch() -> None:
    harness = DifferentialHarness(
        candidate=target(HIGH_BIT_BUGGY_SCRIPT),
        oracle=target(ECHO_SCRIPT),
    )
    mutations = DeterministicByteMutations((b"\x00",))

    campaign = run_fuzz_campaign(
        cases=mutations,
        evaluate=harness.compare,
        max_evaluations=mutations.case_count,
    )

    assert campaign.evaluations == 9
    assert campaign.classification == "product_mismatch"
    assert campaign.failing_case == b"\x80"
    assert campaign.comparison is not None
    assert campaign.comparison.classification == "product_mismatch"


def test_failure_discovery_finds_distinct_real_process_signatures() -> None:
    harness = DifferentialHarness(
        candidate=target(MULTI_BUG_SCRIPT),
        oracle=target(ECHO_SCRIPT),
    )
    corpus = (b"ok", b"out-one", b"out-two", b"exit")

    campaign = run_failure_discovery_campaign(
        cases=corpus.__getitem__,
        evaluate=harness.compare,
        max_evaluations=len(corpus),
        max_unique_failures=4,
    )

    assert campaign.evaluations == 4
    assert campaign.exhausted_budget is True
    assert [failure.case for failure in campaign.failures] == [b"out-one", b"exit"]
    assert [failure.signature.kind for failure in campaign.failures] == [
        "product_mismatch",
        "product_mismatch",
    ]
    assert [failure.signature.dimensions for failure in campaign.failures] == [
        ("stdout",),
        ("exit_code", "stdout"),
    ]


def test_binary_write_fault_adapter_changes_real_process_filesystem_result() -> None:
    harness = DifferentialHarness(
        candidate=target(WRITE_FAULT_SCRIPT),
        oracle=target(ECHO_SCRIPT),
    )

    run = harness.evaluate(b"abcdef")

    assert run.comparison.classification == "product_mismatch"
    assert run.comparison.mismatches == ("stdout",)
    assert run.candidate.stdout.text == "ab"
    assert run.oracle.stdout.text == "abcdef"
    assert run.candidate.infrastructure_error is None
