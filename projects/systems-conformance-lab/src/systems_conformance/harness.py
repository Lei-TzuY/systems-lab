from __future__ import annotations

import hashlib
import json
import os
from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from .comparator import ComparisonResult, compare_results
from .failure import FailureSignature, failure_signature
from .model import ExecutionResult
from .repro import LoadedReproBundle, ReproBundle, load_repro_bundle, write_repro_bundle
from .runner import DEFAULT_MAX_OUTPUT_BYTES, DEFAULT_MAX_TOTAL_OUTPUT_BYTES, run_process


@dataclass(frozen=True, slots=True, init=False)
class CommandTarget:
    """Immutable process-target configuration for differential execution.

    ``argv``, ``cwd``, and ``env`` are normalized at construction time so later
    mutation of caller-owned containers cannot silently change a reproducer.
    ``env=None`` preserves normal environment inheritance; an explicit mapping
    is snapshotted into deterministic key order and replaces the child process
    environment when the target is executed.
    """

    argv: tuple[str, ...]
    cwd: str | None
    env: tuple[tuple[str, str], ...] | None

    def __init__(
        self,
        argv: Sequence[str],
        *,
        cwd: str | os.PathLike[str] | None = None,
        env: Mapping[str, str] | None = None,
    ) -> None:
        normalized_argv = tuple(str(arg) for arg in argv)
        if not normalized_argv:
            raise ValueError("target argv must contain at least one element")

        normalized_cwd = str(Path(cwd)) if cwd is not None else None
        normalized_env = (
            None
            if env is None
            else tuple(sorted((str(key), str(value)) for key, value in env.items()))
        )

        object.__setattr__(self, "argv", normalized_argv)
        object.__setattr__(self, "cwd", normalized_cwd)
        object.__setattr__(self, "env", normalized_env)

    def execute(
        self,
        input_bytes: bytes,
        *,
        timeout_seconds: float,
        max_output_bytes: int,
        max_total_output_bytes: int,
    ) -> ExecutionResult:
        """Execute this target through the shared safe process runner."""
        process_env = None if self.env is None else dict(self.env)
        return run_process(
            self.argv,
            stdin=input_bytes,
            cwd=self.cwd,
            env=process_env,
            timeout_seconds=timeout_seconds,
            max_output_bytes=max_output_bytes,
            max_total_output_bytes=max_total_output_bytes,
        )

    def _replay_identity(self) -> dict[str, object]:
        return {
            "argv": list(self.argv),
            "cwd": self.cwd,
            "env": None if self.env is None else [list(item) for item in self.env],
        }


@dataclass(frozen=True, slots=True)
class DifferentialRun:
    """One complete candidate/oracle execution and classification."""

    candidate: ExecutionResult
    oracle: ExecutionResult
    comparison: ComparisonResult
    signature: FailureSignature | None


@dataclass(frozen=True, slots=True)
class ReproReplay:
    """Observed result of replaying one validated reproducer bundle."""

    bundle: LoadedReproBundle
    run: DifferentialRun

    @property
    def reproduced(self) -> bool:
        """Return whether replay preserved the bundle's stable failure identity."""
        return self.run.signature == self.bundle.signature


@dataclass(frozen=True, slots=True)
class DifferentialHarness:
    """Composable integration boundary over runner, comparator, and repro primitives.

    The harness deliberately owns only target execution and result classification.
    Case generation, mutation, reduction strategy, fault side effects, and corpus
    policy remain adapter-level concerns and compose through the existing generic
    fuzz/reducer/fault APIs.
    """

    candidate: CommandTarget
    oracle: CommandTarget
    timeout_seconds: float = 10.0
    max_output_bytes: int = DEFAULT_MAX_OUTPUT_BYTES
    max_total_output_bytes: int = DEFAULT_MAX_TOTAL_OUTPUT_BYTES

    def __post_init__(self) -> None:
        if self.timeout_seconds <= 0:
            raise ValueError("timeout_seconds must be positive")
        if self.max_output_bytes < 0:
            raise ValueError("max_output_bytes must be non-negative")
        if self.max_total_output_bytes <= 0:
            raise ValueError("max_total_output_bytes must be positive")

    @property
    def replay_context_sha256(self) -> str:
        """Return a non-disclosing fingerprint of execution-affecting configuration."""
        context = {
            "candidate": self.candidate._replay_identity(),
            "oracle": self.oracle._replay_identity(),
            "timeout_seconds": self.timeout_seconds,
            "max_output_bytes": self.max_output_bytes,
            "max_total_output_bytes": self.max_total_output_bytes,
        }
        canonical = json.dumps(
            context,
            sort_keys=True,
            separators=(",", ":"),
            ensure_ascii=False,
        ).encode("utf-8")
        return hashlib.sha256(canonical).hexdigest()

    def evaluate(self, input_bytes: bytes) -> DifferentialRun:
        """Execute both targets and return one stable differential result."""
        candidate = self.candidate.execute(
            input_bytes,
            timeout_seconds=self.timeout_seconds,
            max_output_bytes=self.max_output_bytes,
            max_total_output_bytes=self.max_total_output_bytes,
        )
        oracle = self.oracle.execute(
            input_bytes,
            timeout_seconds=self.timeout_seconds,
            max_output_bytes=self.max_output_bytes,
            max_total_output_bytes=self.max_total_output_bytes,
        )
        comparison = compare_results(candidate, oracle)
        return DifferentialRun(
            candidate=candidate,
            oracle=oracle,
            comparison=comparison,
            signature=failure_signature(comparison),
        )

    def compare(self, input_bytes: bytes) -> ComparisonResult:
        """Compatibility adapter for ``run_fuzz_campaign(evaluate=...)``."""
        return self.evaluate(input_bytes).comparison

    def preserves_failure(
        self,
        input_bytes: bytes,
        expected_signature: FailureSignature,
    ) -> bool:
        """Return whether input reproduces exactly one previously observed failure class."""
        return self.evaluate(input_bytes).signature == expected_signature

    def write_repro(
        self,
        destination: Path,
        *,
        input_bytes: bytes,
        expected_signature: FailureSignature | None = None,
        metadata: dict[str, Any] | None = None,
    ) -> ReproBundle:
        """Re-evaluate a failing case and persist a deterministic repro bundle.

        When ``expected_signature`` is supplied, signature drift is rejected so
        reduction or later reruns cannot accidentally persist a different failure.
        """
        result = self.evaluate(input_bytes)
        if result.signature is None:
            raise ValueError("cannot create a repro bundle for a matching input")
        if expected_signature is not None and result.signature != expected_signature:
            raise ValueError("repro input does not preserve the expected failure signature")

        return write_repro_bundle(
            destination,
            input_bytes=input_bytes,
            candidate=result.candidate,
            oracle=result.oracle,
            comparison=result.comparison,
            signature=result.signature,
            metadata=metadata,
            replay_context_sha256=self.replay_context_sha256,
        )

    def replay_repro(
        self,
        path: Path,
        *,
        max_input_bytes: int = 16 * 1024 * 1024,
        max_manifest_bytes: int = 1024 * 1024,
        require_same_context: bool = True,
    ) -> ReproReplay:
        """Safely load a repro bundle and execute its input against this harness.

        Harness-written bundles carry a SHA-256 fingerprint over the candidate,
        oracle, timeout, capture limit, and hard aggregate output-limit configuration.
        The fingerprint contains no raw argv/env/cwd values. By default replay rejects
        a different context before executing untrusted input. Callers may explicitly
        disable this check when intentionally testing a reproducer against a changed target.
        """
        bundle = load_repro_bundle(
            path,
            max_input_bytes=max_input_bytes,
            max_manifest_bytes=max_manifest_bytes,
        )
        if (
            require_same_context
            and bundle.replay_context_sha256 is not None
            and bundle.replay_context_sha256 != self.replay_context_sha256
        ):
            raise ValueError("repro replay context does not match this harness")
        return ReproReplay(bundle=bundle, run=self.evaluate(bundle.input_bytes))
