from __future__ import annotations

from dataclasses import asdict, dataclass
from typing import Any

SCHEMA_VERSION = "systems-conformance.execution.v1"


@dataclass(frozen=True, slots=True)
class StreamCapture:
    text: str
    total_bytes: int
    truncated: bool


@dataclass(frozen=True, slots=True)
class ExecutionResult:
    argv: tuple[str, ...]
    duration_ms: int
    timed_out: bool
    exit_code: int | None
    signal: int | None
    stdout: StreamCapture
    stderr: StreamCapture
    infrastructure_error: str | None = None
    schema_version: str = SCHEMA_VERSION

    def to_dict(self) -> dict[str, Any]:
        """Return a JSON-serializable execution record."""
        return asdict(self)
