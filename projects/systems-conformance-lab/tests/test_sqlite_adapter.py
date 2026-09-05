from __future__ import annotations

import json

from systems_conformance import DifferentialHarness
from systems_conformance.sqlite_adapter import SQLiteQueryTarget


def _request(*, setup: list[str], query: str, params: list[object] | None = None) -> bytes:
    return json.dumps(
        {"setup": setup, "query": query, "params": [] if params is None else params},
        separators=(",", ":"),
    ).encode()


def _execute(case: bytes):
    return SQLiteQueryTarget().as_command_target().execute(
        case,
        timeout_seconds=2.0,
        max_output_bytes=4096,
        max_total_output_bytes=8192,
    )


def test_sqlite_target_returns_canonical_rows_and_blobs() -> None:
    result = _execute(
        _request(
            setup=[
                "CREATE TABLE items(id INTEGER PRIMARY KEY, name TEXT, payload BLOB)",
                "INSERT INTO items(name, payload) VALUES ('alpha', X'00ff')",
            ],
            query="SELECT id, name, payload FROM items ORDER BY id",
        )
    )

    assert result.infrastructure_error is None
    assert result.exit_code == 0
    assert result.stderr.text == ""
    assert json.loads(result.stdout.text) == {
        "columns": ["id", "name", "payload"],
        "rows": [[1, "alpha", {"$blob": "00ff"}]],
    }


def test_sqlite_target_rejects_attach_without_touching_host_files(tmp_path) -> None:
    result = _execute(
        _request(setup=[], query=f"ATTACH DATABASE '{tmp_path / 'escape.db'}' AS escaped")
    )

    assert result.infrastructure_error is None
    assert result.exit_code == 3
    assert result.stdout.text == ""
    assert result.stderr.text.startswith("sqlite_error: ")
    assert not (tmp_path / "escape.db").exists()


def test_sqlite_target_rejects_duplicate_json_fields() -> None:
    result = _execute(b'{"setup":[],"query":"SELECT 1","query":"SELECT 2"}')

    assert result.infrastructure_error is None
    assert result.exit_code == 2
    assert result.stdout.text == ""
    assert result.stderr.text.strip() == "protocol_error: duplicate JSON object field: query"


def test_sqlite_target_rejects_non_finite_json_constants() -> None:
    result = _execute(b'{"setup":[],"query":"SELECT ?","params":[NaN]}')

    assert result.infrastructure_error is None
    assert result.exit_code == 2
    assert result.stdout.text == ""
    assert result.stderr.text.strip() == (
        "protocol_error: non-finite JSON constant is not supported: NaN"
    )


def test_sqlite_target_rejects_float_overflow_to_infinity() -> None:
    result = _execute(b'{"setup":[],"query":"SELECT ?","params":[1e400]}')

    assert result.infrastructure_error is None
    assert result.exit_code == 2
    assert result.stdout.text == ""
    assert result.stderr.text.strip() == "protocol_error: floating params must be finite"


def test_sqlite_target_rejects_integer_outside_binding_range_without_traceback() -> None:
    result = _execute(_request(setup=[], query="SELECT ?", params=[1 << 63]))

    assert result.infrastructure_error is None
    assert result.exit_code == 2
    assert result.stdout.text == ""
    assert result.stderr.text.strip() == (
        "protocol_error: integer params must fit signed 64-bit SQLite range"
    )


def test_strict_protocol_rejection_is_deterministic_across_real_targets() -> None:
    candidate = SQLiteQueryTarget(foreign_keys=True).as_command_target()
    oracle = SQLiteQueryTarget(foreign_keys=False).as_command_target()
    harness = DifferentialHarness(candidate=candidate, oracle=oracle, timeout_seconds=2.0)
    case = b'{"setup":[],"query":"SELECT ?","params":[Infinity]}'

    run = harness.evaluate(case)

    assert run.candidate.exit_code == 2
    assert run.oracle.exit_code == 2
    assert run.candidate.stderr.text == run.oracle.stderr.text
    assert run.comparison.classification == "match"
    assert run.signature is None


def test_real_sqlite_targets_produce_product_mismatch_for_configuration_difference() -> None:
    candidate = SQLiteQueryTarget(foreign_keys=True).as_command_target()
    oracle = SQLiteQueryTarget(foreign_keys=False).as_command_target()
    harness = DifferentialHarness(candidate=candidate, oracle=oracle, timeout_seconds=2.0)
    case = _request(
        setup=[
            "CREATE TABLE parent(id INTEGER PRIMARY KEY)",
            "CREATE TABLE child(parent_id INTEGER REFERENCES parent(id))",
            "INSERT INTO child(parent_id) VALUES (99)",
        ],
        query="SELECT parent_id FROM child",
    )

    run = harness.evaluate(case)

    assert run.candidate.exit_code == 3
    assert run.oracle.exit_code == 0
    assert run.comparison.classification == "product_mismatch"
    assert run.signature is not None
    assert run.signature.kind == "product_mismatch"
