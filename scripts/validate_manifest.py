#!/usr/bin/env python3
"""Validate systems-lab migration manifest invariants."""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "projects" / "manifest.json"
SHA40 = re.compile(r"^[0-9a-f]{40}$")
ALLOWED_STATUS = {
    "pre-flight",
    "ready-for-import",
    "hold",
    "imported-verified",
    "integration-verified",
}
REQUIRED_FIELDS = {
    "name",
    "source_repository",
    "target_path",
    "layer",
    "status",
    "observed_main_sha",
    "blocker",
}


def fail(message: str) -> None:
    print(f"manifest validation failed: {message}", file=sys.stderr)
    raise SystemExit(1)


def valid_sha(value: object) -> bool:
    return isinstance(value, str) and SHA40.fullmatch(value) is not None


def main() -> None:
    try:
        data = json.loads(MANIFEST.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        fail(f"cannot read {MANIFEST.relative_to(ROOT)}: {exc}")

    if data.get("schema_version") != 1:
        fail("schema_version must be 1")
    if data.get("umbrella") != "Lei-TzuY/systems-lab":
        fail("umbrella must be Lei-TzuY/systems-lab")

    projects = data.get("projects")
    if not isinstance(projects, list) or not projects:
        fail("projects must be a non-empty list")

    names: set[str] = set()
    sources: set[str] = set()
    targets: set[str] = set()

    for index, project in enumerate(projects):
        if not isinstance(project, dict):
            fail(f"projects[{index}] must be an object")

        missing = REQUIRED_FIELDS - project.keys()
        if missing:
            fail(f"projects[{index}] missing fields: {', '.join(sorted(missing))}")

        name = project["name"]
        source = project["source_repository"]
        target = project["target_path"]
        status = project["status"]
        observed = project["observed_main_sha"]
        blocker = project["blocker"]

        if not isinstance(name, str) or not name:
            fail(f"projects[{index}].name must be non-empty")
        if not isinstance(source, str) or source != f"Lei-TzuY/{name}":
            fail(f"{name}: source_repository must be Lei-TzuY/{name}")
        if target != f"projects/{name}":
            fail(f"{name}: target_path must be projects/{name}")
        if status not in ALLOWED_STATUS:
            fail(f"{name}: unsupported status {status!r}")

        if name in names or source in sources or target in targets:
            fail(f"{name}: duplicate project identity/source/target")
        names.add(name)
        sources.add(source)
        targets.add(target)

        if observed is not None and not valid_sha(observed):
            fail(f"{name}: observed_main_sha must be null or a 40-character lowercase hex SHA")

        if status == "hold":
            if not isinstance(blocker, str) or not blocker.strip():
                fail(f"{name}: HOLD entries require a blocker")

        if status == "ready-for-import":
            if blocker is not None:
                fail(f"{name}: READY entry cannot have a blocker")
            if not valid_sha(observed):
                fail(f"{name}: READY entry requires an exact source SHA")
            if project.get("source_ci_conclusion") != "success":
                fail(f"{name}: READY entry requires successful source CI evidence")
            if not isinstance(project.get("source_ci_run_id"), int):
                fail(f"{name}: READY entry requires an integer source_ci_run_id")
            contract = project.get("source_equivalent_ci")
            if not isinstance(contract, list) or not contract or not all(
                isinstance(command, str) and command.strip() for command in contract
            ):
                fail(f"{name}: READY entry requires a non-empty source_equivalent_ci contract")

        if status in {"imported-verified", "integration-verified"}:
            if not (ROOT / target).is_dir():
                fail(f"{name}: verified import status requires an existing target subtree")

    secondary = data.get("secondary_candidates", [])
    if not isinstance(secondary, list):
        fail("secondary_candidates must be a list when present")
    for index, candidate in enumerate(secondary):
        if not isinstance(candidate, dict):
            fail(f"secondary_candidates[{index}] must be an object")
        name = candidate.get("name")
        source = candidate.get("source_repository")
        observed = candidate.get("observed_main_sha")
        if not isinstance(name, str) or not name:
            fail(f"secondary_candidates[{index}].name must be non-empty")
        if source != f"Lei-TzuY/{name}":
            fail(f"{name}: secondary source_repository must be Lei-TzuY/{name}")
        if not valid_sha(observed):
            fail(f"{name}: secondary candidate requires an exact observed source SHA")
        if name in names or source in sources:
            fail(f"{name}: secondary candidate duplicates a core project")

    print(f"validated {len(projects)} core systems projects and {len(secondary)} secondary candidate(s)")


if __name__ == "__main__":
    main()
