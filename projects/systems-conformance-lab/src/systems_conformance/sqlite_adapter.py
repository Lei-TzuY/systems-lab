from __future__ import annotations

import sys
from dataclasses import dataclass

from .harness import CommandTarget


@dataclass(frozen=True, slots=True)
class SQLiteQueryTarget:
    """Process-isolated adapter for deterministic SQLite query conformance.

    Inputs are JSON protocol documents consumed by the bundled worker. Each execution
    uses a fresh in-memory database, so fuzz cases cannot leak database state across
    evaluations. SQL still runs as untrusted target input and remains bounded by the
    shared runner's timeout and output limits.
    """

    foreign_keys: bool = True

    def as_command_target(self) -> CommandTarget:
        """Return the argv-only CommandTarget used by DifferentialHarness."""
        return CommandTarget(
            (
                sys.executable,
                "-m",
                "systems_conformance._sqlite_worker",
                "--foreign-keys" if self.foreign_keys else "--no-foreign-keys",
            )
        )
