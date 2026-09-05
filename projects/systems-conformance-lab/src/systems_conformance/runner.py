from __future__ import annotations

import os
import signal
import subprocess
import threading
import time
from collections.abc import Mapping, Sequence
from pathlib import Path
from typing import BinaryIO

from .model import ExecutionResult, StreamCapture

DEFAULT_MAX_OUTPUT_BYTES = 1024 * 1024
DEFAULT_MAX_TOTAL_OUTPUT_BYTES = 16 * 1024 * 1024
_READ_CHUNK_BYTES = 64 * 1024
_POST_EXIT_DRAIN_SECONDS = 0.1
_POST_CLEANUP_JOIN_SECONDS = 0.5


class _OutputBudget:
    def __init__(self, max_total_bytes: int) -> None:
        self.max_total_bytes = max_total_bytes
        self.total_bytes = 0
        self.exceeded = threading.Event()
        self._lock = threading.Lock()

    def account(self, size: int) -> None:
        with self._lock:
            self.total_bytes += size
            if self.total_bytes > self.max_total_bytes:
                self.exceeded.set()


class _StreamAccumulator:
    def __init__(self, max_capture_bytes: int, budget: _OutputBudget) -> None:
        self.max_capture_bytes = max_capture_bytes
        self.budget = budget
        self.total_bytes = 0
        self._captured = bytearray()

    def feed(self, chunk: bytes) -> None:
        self.total_bytes += len(chunk)
        self.budget.account(len(chunk))
        remaining = self.max_capture_bytes - len(self._captured)
        if remaining > 0:
            self._captured.extend(chunk[:remaining])

    def snapshot(self) -> StreamCapture:
        return StreamCapture(
            text=bytes(self._captured).decode("utf-8", errors="replace"),
            total_bytes=self.total_bytes,
            truncated=self.total_bytes > self.max_capture_bytes,
        )


def _drain_stream(stream: BinaryIO, accumulator: _StreamAccumulator) -> None:
    try:
        while chunk := os.read(stream.fileno(), _READ_CHUNK_BYTES):
            accumulator.feed(chunk)
    except (OSError, ValueError):
        return


def _write_stdin(stream: BinaryIO, data: bytes) -> None:
    try:
        stream.write(data)
        stream.flush()
    except (BrokenPipeError, OSError, ValueError):
        pass
    finally:
        try:
            stream.close()
        except OSError:
            pass


def _terminate_process_tree(
    process: subprocess.Popen[bytes], *, root_may_have_exited: bool = False
) -> None:
    if process.poll() is not None and not root_may_have_exited:
        return

    if os.name == "posix":
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        return

    if os.name == "nt":
        subprocess.run(
            ["taskkill", "/PID", str(process.pid), "/T", "/F"],
            stdin=subprocess.DEVNULL,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            check=False,
            shell=False,
        )
        return

    if process.poll() is None:
        process.kill()


def _join_io_threads(threads: Sequence[threading.Thread], timeout: float) -> bool:
    deadline = time.monotonic() + timeout
    for thread in threads:
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            break
        thread.join(remaining)
    return all(not thread.is_alive() for thread in threads)


def run_process(
    argv: Sequence[str],
    *,
    stdin: bytes = b"",
    cwd: str | os.PathLike[str] | None = None,
    env: Mapping[str, str] | None = None,
    timeout_seconds: float = 10.0,
    max_output_bytes: int = DEFAULT_MAX_OUTPUT_BYTES,
    max_total_output_bytes: int = DEFAULT_MAX_TOTAL_OUTPUT_BYTES,
) -> ExecutionResult:
    """Run one untrusted target without a command shell and return a structured record.

    Stdout and stderr are drained concurrently so a target cannot deadlock by filling a pipe.
    Only ``max_output_bytes`` from each stream are retained in memory. A separate aggregate
    ``max_total_output_bytes`` budget bounds how much output the untrusted process may emit at
    all; exceeding it terminates the process tree and is classified as an infrastructure error.
    Descendants that keep inherited stdio pipes open after the root exits are also bounded and
    classified as infrastructure failures rather than allowing reader threads to hang forever.
    """
    if not argv:
        raise ValueError("argv must contain at least one element")
    if timeout_seconds <= 0:
        raise ValueError("timeout_seconds must be positive")
    if max_output_bytes < 0:
        raise ValueError("max_output_bytes must be non-negative")
    if max_total_output_bytes <= 0:
        raise ValueError("max_total_output_bytes must be positive")

    normalized_argv = tuple(str(arg) for arg in argv)
    normalized_cwd = str(Path(cwd)) if cwd is not None else None
    process_env = None if env is None else {str(key): str(value) for key, value in env.items()}

    popen_kwargs: dict[str, object] = {}
    if os.name == "posix":
        popen_kwargs["start_new_session"] = True
    elif os.name == "nt":
        popen_kwargs["creationflags"] = subprocess.CREATE_NEW_PROCESS_GROUP

    started = time.monotonic()
    timed_out = False
    infrastructure_error: str | None = None

    try:
        process = subprocess.Popen(
            normalized_argv,
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            cwd=normalized_cwd,
            env=process_env,
            shell=False,
            **popen_kwargs,
        )
    except OSError as exc:
        duration_ms = round((time.monotonic() - started) * 1000)
        empty = StreamCapture(text="", total_bytes=0, truncated=False)
        return ExecutionResult(
            argv=normalized_argv,
            duration_ms=duration_ms,
            timed_out=False,
            exit_code=None,
            signal=None,
            stdout=empty,
            stderr=empty,
            infrastructure_error=f"{type(exc).__name__}: {exc}",
        )

    assert process.stdin is not None
    assert process.stdout is not None
    assert process.stderr is not None

    budget = _OutputBudget(max_total_output_bytes)
    stdout_accumulator = _StreamAccumulator(max_output_bytes, budget)
    stderr_accumulator = _StreamAccumulator(max_output_bytes, budget)

    stdout_thread = threading.Thread(
        target=_drain_stream,
        args=(process.stdout, stdout_accumulator),
        daemon=True,
    )
    stderr_thread = threading.Thread(
        target=_drain_stream,
        args=(process.stderr, stderr_accumulator),
        daemon=True,
    )
    stdin_thread = threading.Thread(
        target=_write_stdin,
        args=(process.stdin, stdin),
        daemon=True,
    )
    io_threads = (stdout_thread, stderr_thread, stdin_thread)
    stdout_thread.start()
    stderr_thread.start()
    stdin_thread.start()

    deadline = started + timeout_seconds
    while process.poll() is None:
        if budget.exceeded.is_set():
            infrastructure_error = (
                "OutputLimitExceeded: combined stdout/stderr exceeded "
                f"{max_total_output_bytes} bytes"
            )
            _terminate_process_tree(process)
            break
        if time.monotonic() >= deadline:
            timed_out = True
            _terminate_process_tree(process)
            break
        time.sleep(0.005)

    process.wait()

    if not _join_io_threads(io_threads, _POST_EXIT_DRAIN_SECONDS):
        if infrastructure_error is None and not timed_out:
            infrastructure_error = (
                "ProcessTreeLeak: descendant kept inherited stdio open after root exit"
            )
        _terminate_process_tree(process, root_may_have_exited=True)
        _join_io_threads(io_threads, _POST_CLEANUP_JOIN_SECONDS)

    if infrastructure_error is None and budget.exceeded.is_set():
        infrastructure_error = (
            "OutputLimitExceeded: combined stdout/stderr exceeded "
            f"{max_total_output_bytes} bytes"
        )

    return_code = process.returncode
    stdout_capture = stdout_accumulator.snapshot()
    stderr_capture = stderr_accumulator.snapshot()
    duration_ms = round((time.monotonic() - started) * 1000)
    terminating_signal = -return_code if return_code is not None and return_code < 0 else None
    exit_code = return_code if return_code is not None and return_code >= 0 else None

    return ExecutionResult(
        argv=normalized_argv,
        duration_ms=duration_ms,
        timed_out=timed_out,
        exit_code=exit_code,
        signal=terminating_signal,
        stdout=stdout_capture,
        stderr=stderr_capture,
        infrastructure_error=infrastructure_error,
    )
