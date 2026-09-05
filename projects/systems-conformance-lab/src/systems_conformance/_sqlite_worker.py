from __future__ import annotations

import argparse
import json
import math
import sqlite3
import sys
from collections.abc import Sequence
from typing import Any


class ProtocolError(ValueError):
    pass


def _parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(add_help=False)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--foreign-keys", action="store_true")
    group.add_argument("--no-foreign-keys", action="store_true")
    return parser.parse_args(argv)


def _reject_json_constant(value: str) -> Any:
    raise ProtocolError(f"non-finite JSON constant is not supported: {value}")


def _unique_json_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ProtocolError(f"duplicate JSON object field: {key}")
        result[key] = value
    return result


def _decode_request(raw: bytes) -> tuple[list[str], str, list[Any]]:
    try:
        payload = json.loads(
            raw.decode("utf-8"),
            parse_constant=_reject_json_constant,
            object_pairs_hook=_unique_json_object,
        )
    except (UnicodeDecodeError, json.JSONDecodeError) as exc:
        raise ProtocolError("input must be one UTF-8 JSON document") from exc
    if not isinstance(payload, dict):
        raise ProtocolError("top-level request must be an object")
    if set(payload) - {"setup", "query", "params"}:
        raise ProtocolError("request contains unknown fields")

    setup = payload.get("setup", [])
    query = payload.get("query")
    params = payload.get("params", [])
    if not isinstance(setup, list) or not all(isinstance(item, str) for item in setup):
        raise ProtocolError("setup must be a list of SQL strings")
    if not isinstance(query, str) or not query.strip():
        raise ProtocolError("query must be a non-empty SQL string")
    if not isinstance(params, list):
        raise ProtocolError("params must be a JSON array")
    for value in params:
        if value is None or isinstance(value, (bool, str)):
            continue
        if isinstance(value, int):
            if not -(1 << 63) <= value < (1 << 63):
                raise ProtocolError("integer params must fit signed 64-bit SQLite range")
            continue
        if isinstance(value, float):
            if not math.isfinite(value):
                raise ProtocolError("floating params must be finite")
            continue
        raise ProtocolError("params may only contain JSON scalar values")
    return setup, query, params


def _authorizer(
    action: int,
    _arg1: str | None,
    _arg2: str | None,
    _db: str | None,
    _source: str | None,
) -> int:
    forbidden = {sqlite3.SQLITE_ATTACH, sqlite3.SQLITE_DETACH, sqlite3.SQLITE_PRAGMA}
    return sqlite3.SQLITE_DENY if action in forbidden else sqlite3.SQLITE_OK


def _normalize(value: Any) -> Any:
    if isinstance(value, bytes):
        return {"$blob": value.hex()}
    if value is None or isinstance(value, (int, float, str)):
        return value
    raise TypeError(f"unsupported SQLite result type: {type(value).__name__}")


def _disable_extension_loading(connection: sqlite3.Connection) -> None:
    disable = getattr(connection, "enable_load_extension", None)
    if disable is not None:
        disable(False)


def _run(raw: bytes, *, foreign_keys: bool) -> bytes:
    setup, query, params = _decode_request(raw)
    connection = sqlite3.connect(":memory:")
    try:
        _disable_extension_loading(connection)
        connection.execute(f"PRAGMA foreign_keys = {'ON' if foreign_keys else 'OFF'}")
        connection.set_authorizer(_authorizer)
        for statement in setup:
            connection.execute(statement)
        cursor = connection.execute(query, params)
        columns = [] if cursor.description is None else [item[0] for item in cursor.description]
        rows = [[_normalize(value) for value in row] for row in cursor.fetchall()]
        payload = {"columns": columns, "rows": rows}
        return (
            json.dumps(payload, ensure_ascii=False, separators=(",", ":"), allow_nan=False)
            + "\n"
        ).encode("utf-8")
    finally:
        connection.close()


def main(argv: Sequence[str] | None = None) -> int:
    args = _parse_args(argv)
    try:
        output = _run(sys.stdin.buffer.read(), foreign_keys=args.foreign_keys)
    except ProtocolError as exc:
        sys.stderr.write(f"protocol_error: {exc}\n")
        return 2
    except sqlite3.Error as exc:
        error_name = getattr(exc, "sqlite_errorname", type(exc).__name__)
        sys.stderr.write(f"sqlite_error: {error_name}\n")
        return 3
    except (TypeError, ValueError) as exc:
        sys.stderr.write(f"result_error: {exc}\n")
        return 4
    sys.stdout.buffer.write(output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
