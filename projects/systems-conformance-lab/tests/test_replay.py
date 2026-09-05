import json
import os
import sys

import pytest

from systems_conformance import CommandTarget, DifferentialHarness, load_repro_bundle

ECHO_SCRIPT = (
    "import sys; data = sys.stdin.buffer.read(); sys.stdout.buffer.write(data)"
)
BUGGY_SCRIPT = (
    "import sys; data = sys.stdin.buffer.read(); "
    "sys.stdout.buffer.write(data.replace(b'BUG', b'BAD'))"
)


def target(script: str) -> CommandTarget:
    return CommandTarget((sys.executable, "-c", script))


def test_real_process_repro_round_trip_preserves_failure_identity(tmp_path) -> None:
    harness = DifferentialHarness(candidate=target(BUGGY_SCRIPT), oracle=target(ECHO_SCRIPT))
    observed = harness.evaluate(b"BUG")
    assert observed.signature is not None

    bundle = harness.write_repro(
        tmp_path / "repro",
        input_bytes=b"BUG",
        expected_signature=observed.signature,
        metadata={"source": "replay-integration"},
    )

    replay = harness.replay_repro(bundle.path)

    assert replay.reproduced is True
    assert replay.bundle.input_bytes == b"BUG"
    assert replay.bundle.signature == observed.signature
    assert replay.bundle.metadata == {"source": "replay-integration"}
    assert replay.bundle.replay_context_sha256 == harness.replay_context_sha256
    assert replay.run.comparison.classification == "product_mismatch"


def test_replay_rejects_changed_target_context_before_execution(tmp_path) -> None:
    original = DifferentialHarness(candidate=target(BUGGY_SCRIPT), oracle=target(ECHO_SCRIPT))
    bundle = original.write_repro(tmp_path / "repro", input_bytes=b"BUG")

    changed = DifferentialHarness(candidate=target(ECHO_SCRIPT), oracle=target(ECHO_SCRIPT))

    with pytest.raises(ValueError, match="replay context does not match"):
        changed.replay_repro(bundle.path)


def test_replay_reports_signature_drift_when_context_check_is_explicitly_disabled(
    tmp_path,
) -> None:
    original = DifferentialHarness(candidate=target(BUGGY_SCRIPT), oracle=target(ECHO_SCRIPT))
    bundle = original.write_repro(tmp_path / "repro", input_bytes=b"BUG")

    fixed = DifferentialHarness(candidate=target(ECHO_SCRIPT), oracle=target(ECHO_SCRIPT))
    replay = fixed.replay_repro(bundle.path, require_same_context=False)

    assert replay.reproduced is False
    assert replay.run.comparison.classification == "match"
    assert replay.run.signature is None
    assert replay.bundle.signature.kind == "product_mismatch"


def test_replay_context_hash_does_not_disclose_explicit_environment(tmp_path) -> None:
    secret = "do-not-persist-this-value"
    candidate_env = dict(os.environ)
    candidate_env["REPRO_TEST_SECRET"] = secret
    candidate = CommandTarget(
        (sys.executable, "-c", BUGGY_SCRIPT),
        env=candidate_env,
    )
    oracle = CommandTarget((sys.executable, "-c", ECHO_SCRIPT), env=dict(os.environ))
    harness = DifferentialHarness(candidate=candidate, oracle=oracle)
    bundle = harness.write_repro(tmp_path / "repro", input_bytes=b"BUG")

    manifest_text = bundle.manifest_path.read_text(encoding="utf-8")

    assert secret not in manifest_text
    assert "REPRO_TEST_SECRET" not in manifest_text
    assert harness.replay_context_sha256 in manifest_text


def test_loader_rejects_manifest_input_path_escape_before_execution(tmp_path) -> None:
    harness = DifferentialHarness(candidate=target(BUGGY_SCRIPT), oracle=target(ECHO_SCRIPT))
    bundle = harness.write_repro(tmp_path / "repro", input_bytes=b"BUG")
    manifest = json.loads(bundle.manifest_path.read_text(encoding="utf-8"))
    manifest["input"]["path"] = "../outside.bin"
    bundle.manifest_path.write_text(json.dumps(manifest), encoding="utf-8")

    with pytest.raises(ValueError, match="direct child input.bin"):
        load_repro_bundle(bundle.path)


def test_loader_enforces_input_budget_from_manifest(tmp_path) -> None:
    harness = DifferentialHarness(candidate=target(BUGGY_SCRIPT), oracle=target(ECHO_SCRIPT))
    bundle = harness.write_repro(tmp_path / "repro", input_bytes=b"BUG")

    with pytest.raises(ValueError, match="max_input_bytes"):
        load_repro_bundle(bundle.path, max_input_bytes=2)


def test_loader_keeps_legacy_digestless_v1_bundle_replayable(tmp_path) -> None:
    harness = DifferentialHarness(candidate=target(BUGGY_SCRIPT), oracle=target(ECHO_SCRIPT))
    bundle = harness.write_repro(tmp_path / "repro", input_bytes=b"BUG")
    manifest = json.loads(bundle.manifest_path.read_text(encoding="utf-8"))
    del manifest["input"]["sha256"]
    bundle.manifest_path.write_text(json.dumps(manifest), encoding="utf-8")

    loaded = load_repro_bundle(bundle.path)

    assert loaded.input_bytes == b"BUG"


def test_loader_rejects_invalid_failure_signature_schema(tmp_path) -> None:
    harness = DifferentialHarness(candidate=target(BUGGY_SCRIPT), oracle=target(ECHO_SCRIPT))
    bundle = harness.write_repro(tmp_path / "repro", input_bytes=b"BUG")
    manifest = json.loads(bundle.manifest_path.read_text(encoding="utf-8"))
    manifest["failure_signature"]["schema_version"] = "unknown"
    bundle.manifest_path.write_text(json.dumps(manifest), encoding="utf-8")

    with pytest.raises(ValueError, match="failure signature schema"):
        load_repro_bundle(bundle.path)


def test_loader_rejects_declared_input_size_mismatch(tmp_path) -> None:
    harness = DifferentialHarness(candidate=target(BUGGY_SCRIPT), oracle=target(ECHO_SCRIPT))
    bundle = harness.write_repro(tmp_path / "repro", input_bytes=b"BUG")
    manifest = json.loads(bundle.manifest_path.read_text(encoding="utf-8"))
    manifest["input"]["size_bytes"] = 99
    bundle.manifest_path.write_text(json.dumps(manifest), encoding="utf-8")

    with pytest.raises(ValueError, match="does not match manifest"):
        load_repro_bundle(bundle.path)


def test_loader_rejects_same_size_input_tampering_before_execution(tmp_path) -> None:
    marker = tmp_path / "executed"
    marker_script = (
        f"from pathlib import Path; Path({str(marker)!r}).write_text('ran'); "
        + BUGGY_SCRIPT
    )
    harness = DifferentialHarness(candidate=target(marker_script), oracle=target(ECHO_SCRIPT))
    bundle = harness.write_repro(tmp_path / "repro", input_bytes=b"BUG")
    assert marker.exists()
    marker.unlink()

    bundle.input_path.write_bytes(b"XYZ")

    with pytest.raises(ValueError, match="SHA-256"):
        harness.replay_repro(bundle.path)

    assert not marker.exists()


def test_loader_rejects_malformed_replay_context_hash(tmp_path) -> None:
    harness = DifferentialHarness(candidate=target(BUGGY_SCRIPT), oracle=target(ECHO_SCRIPT))
    bundle = harness.write_repro(tmp_path / "repro", input_bytes=b"BUG")
    manifest = json.loads(bundle.manifest_path.read_text(encoding="utf-8"))
    manifest["replay_context_sha256"] = "not-a-sha256"
    bundle.manifest_path.write_text(json.dumps(manifest), encoding="utf-8")

    with pytest.raises(ValueError, match="replay_context_sha256"):
        load_repro_bundle(bundle.path)
