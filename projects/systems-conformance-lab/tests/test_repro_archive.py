import sys
import zipfile

import pytest

from systems_conformance import (
    CommandTarget,
    DifferentialHarness,
    export_repro_archive,
    import_repro_archive,
)

ECHO_SCRIPT = (
    "import sys; data = sys.stdin.buffer.read(); sys.stdout.buffer.write(data)"
)
BUGGY_SCRIPT = (
    "import sys; data = sys.stdin.buffer.read(); "
    "sys.stdout.buffer.write(data.replace(b'BUG', b'BAD') if b'BUG' in data else data)"
)


def target(script: str) -> CommandTarget:
    return CommandTarget((sys.executable, "-c", script))


def test_export_is_deterministic_and_import_replays_real_targets(tmp_path) -> None:
    harness = DifferentialHarness(candidate=target(BUGGY_SCRIPT), oracle=target(ECHO_SCRIPT))
    original = harness.write_repro(
        tmp_path / "original",
        input_bytes=b"BUG",
        metadata={"source": "archive-integration"},
    )

    first_archive = export_repro_archive(original.path, tmp_path / "first.zip")
    second_archive = export_repro_archive(original.path, tmp_path / "second.zip")

    assert first_archive.read_bytes() == second_archive.read_bytes()

    imported = import_repro_archive(first_archive, tmp_path / "imported")
    assert imported.input_path.read_bytes() == b"BUG"
    assert imported.manifest_path.read_bytes() == original.manifest_path.read_bytes()

    replay = harness.replay_repro(imported.path)
    assert replay.reproduced
    assert replay.run.signature == replay.bundle.signature
    assert replay.bundle.metadata == {"source": "archive-integration"}


def test_import_rejects_unexpected_member_before_destination_creation(tmp_path) -> None:
    archive_path = tmp_path / "bad.zip"
    with zipfile.ZipFile(archive_path, mode="w", compression=zipfile.ZIP_STORED) as archive:
        archive.writestr("input.bin", b"case")
        archive.writestr("manifest.json", b"{}")
        archive.writestr("../escape", b"nope")

    destination = tmp_path / "imported"
    with pytest.raises(ValueError, match="exactly input.bin and manifest.json"):
        import_repro_archive(archive_path, destination)

    assert not destination.exists()
    assert not (tmp_path.parent / "escape").exists()


def test_import_rejects_compressed_members_before_destination_creation(tmp_path) -> None:
    archive_path = tmp_path / "compressed.zip"
    with zipfile.ZipFile(archive_path, mode="w", compression=zipfile.ZIP_DEFLATED) as archive:
        archive.writestr("input.bin", b"case")
        archive.writestr("manifest.json", b"{}")

    destination = tmp_path / "imported"
    with pytest.raises(ValueError, match="ZIP_STORED"):
        import_repro_archive(archive_path, destination)

    assert not destination.exists()


def test_import_rejects_invalid_bundle_before_publication(tmp_path) -> None:
    archive_path = tmp_path / "invalid.zip"
    with zipfile.ZipFile(archive_path, mode="w", compression=zipfile.ZIP_STORED) as archive:
        archive.writestr("input.bin", b"case")
        archive.writestr("manifest.json", b"{}")

    destination = tmp_path / "imported"
    with pytest.raises(ValueError, match="unsupported repro bundle schema"):
        import_repro_archive(archive_path, destination)

    assert not destination.exists()
    assert not list(tmp_path.glob(".imported.import-*"))
