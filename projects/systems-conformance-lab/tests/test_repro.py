import json

import pytest

from systems_conformance.comparator import compare_results
from systems_conformance.failure import failure_signature
from systems_conformance.model import ExecutionResult, StreamCapture
from systems_conformance.repro import write_repro_bundle


def _result(stdout: str) -> ExecutionResult:
    encoded = stdout.encode()
    return ExecutionResult(
        argv=("tool",),
        duration_ms=1,
        timed_out=False,
        exit_code=0,
        signal=None,
        stdout=StreamCapture(stdout, len(encoded), False),
        stderr=StreamCapture("", 0, False),
    )


def test_write_repro_bundle_preserves_input_and_manifest(tmp_path):
    candidate = _result("candidate")
    oracle = _result("oracle")
    comparison = compare_results(candidate, oracle)
    signature = failure_signature(comparison)
    assert signature is not None

    bundle = write_repro_bundle(
        tmp_path / "case",
        input_bytes=b"\x00source\xff",
        candidate=candidate,
        oracle=oracle,
        comparison=comparison,
        signature=signature,
        metadata={"adapter": "example"},
    )

    assert bundle.input_path.read_bytes() == b"\x00source\xff"
    manifest = json.loads(bundle.manifest_path.read_text(encoding="utf-8"))
    assert manifest["input"] == {
        "path": "input.bin",
        "sha256": "ac086904e5eec590d28871f81ca952b150ba1d77aca8fcfbdd2404d70df4633b",
        "size_bytes": 8,
    }
    assert manifest["comparison"]["classification"] == "product_mismatch"
    assert manifest["failure_signature"] == {
        "dimensions": list(signature.dimensions),
        "kind": signature.kind,
        "schema_version": signature.schema_version,
    }
    assert manifest["metadata"] == {"adapter": "example"}


def test_manifest_is_deterministic_across_destinations(tmp_path):
    candidate = _result("candidate")
    oracle = _result("oracle")
    comparison = compare_results(candidate, oracle)
    signature = failure_signature(comparison)
    assert signature is not None

    first = write_repro_bundle(
        tmp_path / "one",
        input_bytes=b"case",
        candidate=candidate,
        oracle=oracle,
        comparison=comparison,
        signature=signature,
    )
    second = write_repro_bundle(
        tmp_path / "two",
        input_bytes=b"case",
        candidate=candidate,
        oracle=oracle,
        comparison=comparison,
        signature=signature,
    )

    assert first.manifest_path.read_bytes() == second.manifest_path.read_bytes()


def test_existing_destination_is_never_overwritten(tmp_path):
    destination = tmp_path / "case"
    destination.mkdir()
    marker = destination / "keep"
    marker.write_text("evidence", encoding="utf-8")

    candidate = _result("candidate")
    oracle = _result("oracle")
    comparison = compare_results(candidate, oracle)
    signature = failure_signature(comparison)
    assert signature is not None

    with pytest.raises(FileExistsError):
        write_repro_bundle(
            destination,
            input_bytes=b"case",
            candidate=candidate,
            oracle=oracle,
            comparison=comparison,
            signature=signature,
        )

    assert marker.read_text(encoding="utf-8") == "evidence"


def test_matching_comparison_is_rejected_without_creating_directory(tmp_path):
    comparison = compare_results(_result("same"), _result("same"))
    failing = compare_results(_result("candidate"), _result("oracle"))
    signature = failure_signature(failing)
    assert signature is not None
    destination = tmp_path / "case"

    with pytest.raises(ValueError):
        write_repro_bundle(
            destination,
            input_bytes=b"case",
            candidate=_result("same"),
            oracle=_result("same"),
            comparison=comparison,
            signature=signature,
        )

    assert not destination.exists()
