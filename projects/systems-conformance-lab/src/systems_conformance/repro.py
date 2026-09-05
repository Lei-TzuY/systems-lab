import hashlib
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .comparator import ComparisonResult
from .failure import FAILURE_SIGNATURE_SCHEMA_VERSION, FailureSignature
from .model import ExecutionResult

REPRO_BUNDLE_SCHEMA_VERSION = "systems-conformance.repro-bundle.v1"
DEFAULT_MAX_REPRO_INPUT_BYTES = 16 * 1024 * 1024
DEFAULT_MAX_REPRO_MANIFEST_BYTES = 1024 * 1024


@dataclass(frozen=True, slots=True)
class ReproBundle:
    """Description of one deterministic on-disk reproducer bundle."""

    path: Path
    manifest_path: Path
    input_path: Path
    schema_version: str = REPRO_BUNDLE_SCHEMA_VERSION


@dataclass(frozen=True, slots=True)
class LoadedReproBundle:
    """Validated replay inputs loaded from one reproducer bundle."""

    path: Path
    input_bytes: bytes
    signature: FailureSignature
    metadata: dict[str, Any]
    replay_context_sha256: str | None = None
    schema_version: str = REPRO_BUNDLE_SCHEMA_VERSION


def _require_regular_file(path: Path, *, label: str) -> None:
    if path.is_symlink() or not path.is_file():
        raise ValueError(f"{label} must be a regular file: {path}")


def _load_failure_signature(value: object) -> FailureSignature:
    if not isinstance(value, dict):
        raise TypeError("failure_signature must be an object")
    if value.get("schema_version") != FAILURE_SIGNATURE_SCHEMA_VERSION:
        raise ValueError("unsupported failure signature schema")

    kind = value.get("kind")
    if kind not in {"product_mismatch", "infrastructure_failure"}:
        raise ValueError("invalid failure signature kind")

    dimensions = value.get("dimensions")
    if not isinstance(dimensions, list) or not all(
        isinstance(item, str) for item in dimensions
    ):
        raise ValueError("failure signature dimensions must be a list of strings")

    return FailureSignature(kind=kind, dimensions=tuple(dimensions))


def _load_sha256(value: object, *, label: str) -> str:
    if not isinstance(value, str):
        raise TypeError(f"{label} must be a string")
    if len(value) != 64 or any(character not in "0123456789abcdef" for character in value):
        raise ValueError(f"{label} must be a lowercase SHA-256 hex digest")
    return value


def _load_replay_context_sha256(value: object) -> str | None:
    if value is None:
        return None
    return _load_sha256(value, label="replay_context_sha256")


def load_repro_bundle(
    path: Path,
    *,
    max_input_bytes: int = DEFAULT_MAX_REPRO_INPUT_BYTES,
    max_manifest_bytes: int = DEFAULT_MAX_REPRO_MANIFEST_BYTES,
) -> LoadedReproBundle:
    """Load and validate a repro bundle before replaying untrusted evidence.

    Replay accepts only the deterministic v1 layout emitted by
    :func:`write_repro_bundle`: one direct-child ``manifest.json`` and
    ``input.bin``. Symlinks, oversized artifacts, schema drift, declared
    input-size mismatches, and present input-content digest mismatches are
    rejected before execution. Older v1 bundles without a digest remain
    loadable for replay compatibility.
    """

    if max_input_bytes < 0:
        raise ValueError("max_input_bytes must be non-negative")
    if max_manifest_bytes <= 0:
        raise ValueError("max_manifest_bytes must be positive")

    path = Path(path)
    if path.is_symlink() or not path.is_dir():
        raise ValueError(f"repro bundle path must be a directory: {path}")

    manifest_path = path / "manifest.json"
    input_path = path / "input.bin"
    _require_regular_file(manifest_path, label="repro manifest")
    _require_regular_file(input_path, label="repro input")

    manifest_size = manifest_path.stat().st_size
    if manifest_size > max_manifest_bytes:
        raise ValueError("repro manifest exceeds max_manifest_bytes")

    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ValueError("repro manifest is not valid UTF-8 JSON") from exc

    if not isinstance(manifest, dict):
        raise TypeError("repro manifest must be an object")
    if manifest.get("schema_version") != REPRO_BUNDLE_SCHEMA_VERSION:
        raise ValueError("unsupported repro bundle schema")

    input_record = manifest.get("input")
    if not isinstance(input_record, dict):
        raise TypeError("repro input metadata must be an object")
    if input_record.get("path") != "input.bin":
        raise ValueError("repro input path must be the direct child input.bin")

    declared_size = input_record.get("size_bytes")
    if not isinstance(declared_size, int) or isinstance(declared_size, bool):
        raise TypeError("repro input size_bytes must be an integer")
    if declared_size < 0:
        raise ValueError("repro input size_bytes must be non-negative")
    if declared_size > max_input_bytes:
        raise ValueError("repro input exceeds max_input_bytes")
    if input_path.stat().st_size != declared_size:
        raise ValueError("repro input size does not match manifest metadata")

    expected_sha256_value = input_record.get("sha256")
    expected_sha256 = None
    if expected_sha256_value is not None:
        expected_sha256 = _load_sha256(
            expected_sha256_value,
            label="repro input sha256",
        )
    input_bytes = input_path.read_bytes()
    if len(input_bytes) != declared_size:
        raise ValueError("repro input changed while being loaded")
    if (
        expected_sha256 is not None
        and hashlib.sha256(input_bytes).hexdigest() != expected_sha256
    ):
        raise ValueError("repro input SHA-256 does not match manifest metadata")

    for field in ("candidate", "oracle", "comparison"):
        if not isinstance(manifest.get(field), dict):
            raise TypeError(f"repro {field} record must be an object")

    metadata = manifest.get("metadata")
    if not isinstance(metadata, dict):
        raise TypeError("repro metadata must be an object")

    return LoadedReproBundle(
        path=path,
        input_bytes=input_bytes,
        signature=_load_failure_signature(manifest.get("failure_signature")),
        metadata=metadata,
        replay_context_sha256=_load_replay_context_sha256(
            manifest.get("replay_context_sha256")
        ),
    )


def write_repro_bundle(
    destination: Path,
    *,
    input_bytes: bytes,
    candidate: ExecutionResult,
    oracle: ExecutionResult,
    comparison: ComparisonResult,
    signature: FailureSignature,
    metadata: dict[str, Any] | None = None,
    replay_context_sha256: str | None = None,
) -> ReproBundle:
    """Write a self-contained, deterministic reproducer bundle.

    Existing destinations are rejected so evidence cannot be silently
    overwritten. The manifest deliberately uses relative artifact names and
    sorted JSON keys, while the original input is retained byte-for-byte and
    bound to its manifest with a SHA-256 content digest.
    """

    destination = Path(destination)
    if destination.exists():
        raise FileExistsError(f"repro bundle destination already exists: {destination}")
    if comparison.equivalent:
        raise ValueError("cannot create a repro bundle for a matching comparison")
    if metadata is not None and not isinstance(metadata, dict):
        raise TypeError("metadata must be a dict or None")
    replay_context_sha256 = _load_replay_context_sha256(replay_context_sha256)

    destination.mkdir(parents=True)
    input_path = destination / "input.bin"
    manifest_path = destination / "manifest.json"

    try:
        input_path.write_bytes(input_bytes)
        manifest = {
            "schema_version": REPRO_BUNDLE_SCHEMA_VERSION,
            "input": {
                "path": input_path.name,
                "sha256": hashlib.sha256(input_bytes).hexdigest(),
                "size_bytes": len(input_bytes),
            },
            "candidate": candidate.to_dict(),
            "oracle": oracle.to_dict(),
            "comparison": comparison.to_dict(),
            "failure_signature": signature.to_dict(),
            "metadata": {} if metadata is None else metadata,
        }
        if replay_context_sha256 is not None:
            manifest["replay_context_sha256"] = replay_context_sha256
        manifest_path.write_text(
            json.dumps(manifest, indent=2, sort_keys=True, ensure_ascii=False) + "\n",
            encoding="utf-8",
            newline="\n",
        )
    except BaseException:
        # Avoid leaving a partially valid-looking bundle behind.
        if manifest_path.exists():
            manifest_path.unlink()
        if input_path.exists():
            input_path.unlink()
        try:
            destination.rmdir()
        except OSError:
            pass
        raise

    return ReproBundle(
        path=destination,
        manifest_path=manifest_path,
        input_path=input_path,
    )
