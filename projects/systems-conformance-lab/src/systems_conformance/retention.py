import json
import shutil
from dataclasses import dataclass
from pathlib import Path

from .repro import REPRO_BUNDLE_SCHEMA_VERSION


@dataclass(frozen=True, slots=True)
class RetentionResult:
    """Summary of one deterministic repro-bundle retention pass."""

    kept: tuple[Path, ...]
    removed: tuple[Path, ...]
    ignored: tuple[Path, ...]


def enforce_repro_retention(root: Path, *, max_bundles: int) -> RetentionResult:
    """Keep at most ``max_bundles`` valid repro bundles below ``root``.

    Only direct child directories that look like bundles produced by this
    package are eligible for deletion. Symlinks, malformed directories, and
    unknown schema versions are ignored. Eligible bundles are ordered newest
    first by manifest mtime, with directory name as a deterministic tiebreaker.
    """

    if isinstance(max_bundles, bool) or not isinstance(max_bundles, int):
        raise TypeError("max_bundles must be an int")
    if max_bundles < 0:
        raise ValueError("max_bundles must be non-negative")

    root = Path(root)
    if not root.exists():
        return RetentionResult(kept=(), removed=(), ignored=())
    if not root.is_dir():
        raise NotADirectoryError(root)

    eligible: list[tuple[int, str, Path]] = []
    ignored: list[Path] = []

    for child in sorted(root.iterdir(), key=lambda path: path.name):
        if child.is_symlink() or not child.is_dir():
            ignored.append(child)
            continue

        manifest_path = child / "manifest.json"
        input_path = child / "input.bin"
        if (
            manifest_path.is_symlink()
            or input_path.is_symlink()
            or not manifest_path.is_file()
            or not input_path.is_file()
        ):
            ignored.append(child)
            continue

        try:
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        except (OSError, UnicodeError, json.JSONDecodeError):
            ignored.append(child)
            continue

        if not isinstance(manifest, dict):
            ignored.append(child)
            continue
        if manifest.get("schema_version") != REPRO_BUNDLE_SCHEMA_VERSION:
            ignored.append(child)
            continue
        input_record = manifest.get("input")
        if not isinstance(input_record, dict) or input_record.get("path") != "input.bin":
            ignored.append(child)
            continue
        expected_size = input_record.get("size_bytes")
        if (
            isinstance(expected_size, bool)
            or not isinstance(expected_size, int)
            or expected_size < 0
        ):
            ignored.append(child)
            continue

        try:
            if input_path.stat().st_size != expected_size:
                ignored.append(child)
                continue
            manifest_mtime_ns = manifest_path.stat().st_mtime_ns
        except OSError:
            ignored.append(child)
            continue
        eligible.append((manifest_mtime_ns, child.name, child))

    eligible.sort(key=lambda item: (-item[0], item[1]))
    kept = [item[2] for item in eligible[:max_bundles]]
    removed = [item[2] for item in eligible[max_bundles:]]

    for path in removed:
        shutil.rmtree(path)

    return RetentionResult(
        kept=tuple(kept),
        removed=tuple(removed),
        ignored=tuple(ignored),
    )
