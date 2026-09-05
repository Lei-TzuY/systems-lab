import errno
import io

import pytest

from systems_conformance import FaultingBinaryWriter, FaultSpec


def test_short_write_triggers_once_at_selected_occurrence() -> None:
    sink = io.BytesIO()
    writer = FaultingBinaryWriter(
        sink,
        FaultSpec(operation="write", occurrence=1, kind="short_write"),
        short_write_bytes=2,
    )

    assert writer.write(b"first") == 5
    assert writer.write(b"second") == 2
    assert writer.triggered is True
    assert writer.write(b"third") == 5
    assert sink.getvalue() == b"firstsethird"


def test_io_error_does_not_touch_underlying_stream() -> None:
    sink = io.BytesIO()
    writer = FaultingBinaryWriter(
        sink,
        FaultSpec(operation="write", occurrence=0, kind="io_error"),
    )

    with pytest.raises(OSError) as exc_info:
        writer.write(b"payload")

    assert exc_info.value.errno == errno.EIO
    assert sink.getvalue() == b""
    assert writer.triggered is True
    assert writer.write(b"recovery") == 8
    assert sink.getvalue() == b"recovery"


@pytest.mark.parametrize(
    ("spec", "short_write_bytes", "message"),
    [
        (FaultSpec("read", 0, "io_error"), 0, "operation='write'"),
        (FaultSpec("write", 0, "drop"), 0, "unsupported binary write fault kind"),
        (FaultSpec("write", 0, "short_write"), -1, "must be non-negative"),
    ],
)
def test_invalid_write_fault_configuration_fails_closed(
    spec: FaultSpec,
    short_write_bytes: int,
    message: str,
) -> None:
    with pytest.raises(ValueError, match=message):
        FaultingBinaryWriter(
            io.BytesIO(),
            spec,
            short_write_bytes=short_write_bytes,
        )
