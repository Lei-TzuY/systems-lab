#!/usr/bin/env python3
"""Validate systems-lab cross-project integration evidence."""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PROJECT_MANIFEST = ROOT / "projects" / "manifest.json"
INTEGRATION_MANIFEST = ROOT / "integrations" / "manifest.json"
SHA40 = re.compile(r"^[0-9a-f]{40}$")
REQUIRED_FIELDS = {
    "name",
    "status",
    "participants",
    "participant_source_shas",
    "target_path",
    "workflow_path",
    "scope",
    "pr_number",
    "pr_head_sha",
    "pr_manifest_run_id",
    "pr_verification_run_id",
    "merged_umbrella_sha",
    "umbrella_post_merge_manifest_run_id",
    "umbrella_post_merge_verification_run_id",
    "verification_contract",
    "limitations",
}
RUN_ID_FIELDS = {
    "pr_manifest_run_id",
    "pr_verification_run_id",
    "umbrella_post_merge_manifest_run_id",
    "umbrella_post_merge_verification_run_id",
}


def fail(message: str) -> None:
    print(f"integration manifest validation failed: {message}", file=sys.stderr)
    raise SystemExit(1)


def load_json(path: Path) -> dict[str, object]:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        fail(f"cannot read {path.relative_to(ROOT)}: {exc}")
    if not isinstance(data, dict):
        fail(f"{path.relative_to(ROOT)} must contain a JSON object")
    return data


def valid_sha(value: object) -> bool:
    return isinstance(value, str) and SHA40.fullmatch(value) is not None


def nonempty_strings(value: object) -> bool:
    return (
        isinstance(value, list)
        and bool(value)
        and all(isinstance(item, str) and item.strip() for item in value)
    )


def main() -> None:
    projects_data = load_json(PROJECT_MANIFEST)
    integrations_data = load_json(INTEGRATION_MANIFEST)

    if integrations_data.get("schema_version") != 1:
        fail("schema_version must be 1")
    if integrations_data.get("umbrella") != "Lei-TzuY/systems-lab":
        fail("umbrella must be Lei-TzuY/systems-lab")

    projects = projects_data.get("projects")
    if not isinstance(projects, list) or not projects:
        fail("project manifest must contain core projects")

    project_by_name: dict[str, dict[str, object]] = {}
    for project in projects:
        if not isinstance(project, dict):
            fail("project manifest contains a non-object project")
        name = project.get("name")
        if isinstance(name, str):
            project_by_name[name] = project

    integrations = integrations_data.get("verified_integrations")
    if not isinstance(integrations, list) or not integrations:
        fail("verified_integrations must be a non-empty list")

    names: set[str] = set()
    targets: set[str] = set()

    for index, integration in enumerate(integrations):
        if not isinstance(integration, dict):
            fail(f"verified_integrations[{index}] must be an object")

        missing = REQUIRED_FIELDS - integration.keys()
        if missing:
            fail(
                f"verified_integrations[{index}] missing fields: "
                + ", ".join(sorted(missing))
            )

        name = integration["name"]
        status = integration["status"]
        participants = integration["participants"]
        source_shas = integration["participant_source_shas"]
        target = integration["target_path"]
        workflow = integration["workflow_path"]

        if not isinstance(name, str) or not name:
            fail(f"verified_integrations[{index}].name must be non-empty")
        if name in names:
            fail(f"duplicate integration name: {name}")
        names.add(name)

        if status != "integration-verified":
            fail(f"{name}: status must be integration-verified")

        if (
            not isinstance(participants, list)
            or len(participants) < 2
            or not all(isinstance(item, str) and item for item in participants)
            or len(set(participants)) != len(participants)
        ):
            fail(f"{name}: participants must contain at least two unique project names")

        if not isinstance(source_shas, dict) or set(source_shas) != set(participants):
            fail(f"{name}: participant_source_shas must exactly match participants")

        for participant in participants:
            project = project_by_name.get(participant)
            if project is None:
                fail(f"{name}: unknown participant {participant}")
            if project.get("status") not in {"imported-verified", "integration-verified"}:
                fail(f"{name}: participant {participant} is not a verified import")
            imported_source = project.get("imported_source_sha")
            if not valid_sha(imported_source):
                fail(f"{name}: participant {participant} lacks an imported source SHA")
            if source_shas.get(participant) != imported_source:
                fail(f"{name}: participant {participant} source SHA does not match project ledger")

        if not isinstance(target, str) or not target.startswith("integrations/"):
            fail(f"{name}: target_path must live under integrations/")
        if target in targets:
            fail(f"duplicate integration target_path: {target}")
        targets.add(target)
        if not (ROOT / target).is_dir():
            fail(f"{name}: target_path does not exist")

        if not isinstance(workflow, str) or not workflow.startswith(".github/workflows/"):
            fail(f"{name}: workflow_path must live under .github/workflows/")
        if not (ROOT / workflow).is_file():
            fail(f"{name}: workflow_path does not exist")

        if not isinstance(integration["scope"], str) or not integration["scope"].strip():
            fail(f"{name}: scope must be non-empty")
        if not nonempty_strings(integration["verification_contract"]):
            fail(f"{name}: verification_contract must be a non-empty string list")
        if not nonempty_strings(integration["limitations"]):
            fail(f"{name}: limitations must be a non-empty string list")

        if not isinstance(integration["pr_number"], int) or integration["pr_number"] <= 0:
            fail(f"{name}: pr_number must be a positive integer")
        if not valid_sha(integration["pr_head_sha"]):
            fail(f"{name}: pr_head_sha must be an exact lowercase SHA")
        if not valid_sha(integration["merged_umbrella_sha"]):
            fail(f"{name}: merged_umbrella_sha must be an exact lowercase SHA")

        for field in RUN_ID_FIELDS:
            value = integration[field]
            if not isinstance(value, int) or value <= 0:
                fail(f"{name}: {field} must be a positive integer")

    print(f"validated {len(integrations)} verified cross-project integration(s)")


if __name__ == "__main__":
    main()
