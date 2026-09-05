import os
import shutil
import tempfile
import zipfile
from pathlib import Path

from .repro import (
    DEFAULT_MAX_REPRO_INPUT_BYTES,
    DEFAULT_MAX_REPRO_MANIFEST_BYTES,
    ReproBundle,
    load_repro_bundle,
)

REPRO_ARCHIVE_MEMBERS = ("input.bin", "manifest.json")
DEFAULT_MAX_REPRO_ARCHIVE_BYTES = (
    DEFAULT_MAX_REPRO_INPUT_BYTES + DEFAULT_MAX_REPRO_MANIFEST_BYTES + 4096
)
_FIXED_ZIP_TIMESTAMP = (1980, 1, 1, 0, 0, 0)


def _regular_zip_info(name: str) -> zipfile.ZipInfo:
    info = zipfile.ZipInfo(name, date_time=_FIXED_ZIP_TIMESTAMP)
    info.compress_type = zipfile.ZIP_STORED
    info.create_system = 3
    info.external_attr = 0o100644 << 16
    return info


def export_repro_archive(
    bundle_path: Path,
    archive_path: Path,
    *,
    max_input_bytes: int = DEFAULT_MAX_REPRO_INPUT_BYTES,
    max_manifest_bytes: int = DEFAULT_MAX_REPRO_MANIFEST_BYTES,
) -> Path:
    """Export one validated repro bundle as a deterministic portable ZIP.

    The archive contains exactly the existing ``input.bin`` and
    ``manifest.json`` bytes. ZIP_STORED plus fixed member metadata keeps equal
    bundles byte-for-byte reproducible across export locations while avoiding
    decompression bombs on the supported import path.
    """

    bundle_path = Path(bundle_path)
    archive_path = Path(archive_path)
    if archive_path.exists():
        raise FileExistsError(f"repro archive destination already exists: {archive_path}")

    load_repro_bundle(
        bundle_path,
        max_input_bytes=max_input_bytes,
        max_manifest_bytes=max_manifest_bytes,
    )
    manifest = (bundle_path / "manifest.json").read_bytes()
    input_bytes = (bundle_path / "input.bin").read_bytes()

    archive_path.parent.mkdir(parents=True, exist_ok=True)
    try:
        with zipfile.ZipFile(
            archive_path,
            mode="x",
            compression=zipfile.ZIP_STORED,
            allowZip64=False,
        ) as archive:
            archive.writestr(_regular_zip_info("input.bin"), input_bytes)
            archive.writestr(_regular_zip_info("manifest.json"), manifest)
    except BaseException:
        if archive_path.exists():
            archive_path.unlink()
        raise

    return archive_path


def import_repro_archive(
    archive_path: Path,
    destination: Path,
    *,
    max_input_bytes: int = DEFAULT_MAX_REPRO_INPUT_BYTES,
    max_manifest_bytes: int = DEFAULT_MAX_REPRO_MANIFEST_BYTES,
    max_archive_bytes: int = DEFAULT_MAX_REPRO_ARCHIVE_BYTES,
) -> ReproBundle:
    """Import a deterministic repro archive through the normal bundle validator.

    Only the two direct-child regular members emitted by
    :func:`export_repro_archive` are accepted. Unexpected paths, duplicates,
    encryption, compression-method drift, oversized artifacts, and invalid
    bundle contents are rejected before the destination becomes visible.
    """

    if max_archive_bytes <= 0:
        raise ValueError("max_archive_bytes must be positive")

    archive_path = Path(archive_path)
    destination = Path(destination)
    if archive_path.is_symlink() or not archive_path.is_file():
        raise ValueError(f"repro archive must be a regular file: {archive_path}")
    if archive_path.stat().st_size > max_archive_bytes:
        raise ValueError("repro archive exceeds max_archive_bytes")
    if destination.exists():
        raise FileExistsError(f"repro bundle destination already exists: {destination}")

    with zipfile.ZipFile(archive_path, mode="r") as archive:
        members = archive.infolist()
        names = [member.filename for member in members]
        if len(members) != len(REPRO_ARCHIVE_MEMBERS) or set(names) != set(
            REPRO_ARCHIVE_MEMBERS
        ):
            raise ValueError("repro archive must contain exactly input.bin and manifest.json")
        if len(names) != len(set(names)):
            raise ValueError("repro archive contains duplicate members")

        by_name = {member.filename: member for member in members}
        for member in members:
            if member.is_dir():
                raise ValueError("repro archive members must be regular files")
            if member.flag_bits & 0x1:
                raise ValueError("encrypted repro archive members are not supported")
            if member.compress_type != zipfile.ZIP_STORED:
                raise ValueError("repro archive members must use ZIP_STORED")

        if by_name["input.bin"].file_size > max_input_bytes:
            raise ValueError("repro input exceeds max_input_bytes")
        if by_name["manifest.json"].file_size > max_manifest_bytes:
            raise ValueError("repro manifest exceeds max_manifest_bytes")

        input_bytes = archive.read("input.bin")
        manifest_bytes = archive.read("manifest.json")

    destination.parent.mkdir(parents=True, exist_ok=True)
    staging = Path(
        tempfile.mkdtemp(
            prefix=f".{destination.name}.import-",
            dir=destination.parent,
        )
    )
    published = False
    try:
        (staging / "input.bin").write_bytes(input_bytes)
        (staging / "manifest.json").write_bytes(manifest_bytes)
        load_repro_bundle(
            staging,
            max_input_bytes=max_input_bytes,
            max_manifest_bytes=max_manifest_bytes,
        )
        if destination.exists():
            raise FileExistsError(
                f"repro bundle destination already exists: {destination}"
            )
        os.rename(staging, destination)
        published = True
    finally:
        if not published and staging.exists():
            shutil.rmtree(staging)

    return ReproBundle(
        path=destination,
        manifest_path=destination / "manifest.json",
        input_path=destination / "input.bin",
    )
