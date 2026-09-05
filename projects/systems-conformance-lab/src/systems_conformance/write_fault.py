import errno
from typing import BinaryIO

from .fault import FaultController, FaultSpec


class FaultingBinaryWriter:
    """Inject one deterministic write fault into a binary stream.

    The adapter is deliberately narrow: it models a target's binary ``write``
    boundary and supports only ``short_write`` and ``io_error`` effects. The
    underlying stream remains owned by the caller.
    """

    __slots__ = ("_controller", "_short_write_bytes", "_sink")

    def __init__(
        self,
        sink: BinaryIO,
        spec: FaultSpec,
        *,
        short_write_bytes: int = 0,
    ) -> None:
        if spec.operation != "write":
            raise ValueError("binary writer only supports operation='write'")
        if spec.kind not in {"short_write", "io_error"}:
            raise ValueError(f"unsupported binary write fault kind: {spec.kind}")
        if short_write_bytes < 0:
            raise ValueError("short_write_bytes must be non-negative")

        self._sink = sink
        self._controller = FaultController(spec)
        self._short_write_bytes = short_write_bytes

    @property
    def triggered(self) -> bool:
        return self._controller.triggered

    def write(self, data: bytes | bytearray | memoryview) -> int:
        """Write bytes, applying the configured fault at its occurrence."""

        view = memoryview(data).cast("B")
        fault = self._controller.checkpoint("write")
        if fault is None:
            return self._sink.write(view)

        if fault.kind == "io_error":
            raise OSError(errno.EIO, "injected binary write fault")

        if not view:
            return 0
        count = min(self._short_write_bytes, len(view) - 1)
        return self._sink.write(view[:count])
