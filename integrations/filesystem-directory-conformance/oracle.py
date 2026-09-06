from __future__ import annotations

import sys
import zlib

HEADER_LEN = 40
NAME_MAX = 255
CRC_OFFSET = 32


def record_crc(data: bytes) -> int:
    normalized = bytearray(data)
    if len(normalized) >= CRC_OFFSET + 4:
        normalized[CRC_OFFSET : CRC_OFFSET + 4] = b"\x00\x00\x00\x00"
    return zlib.crc32(normalized) & 0xFFFFFFFF


def canonical(data: bytes) -> str:
    if len(data) < HEADER_LEN:
        return "err|unexpected-eof"
    if data[:4] != b"DNT1":
        return "err|invalid-data"
    if int.from_bytes(data[4:6], "little") != 1:
        return "err|invalid-data"
    if (
        int.from_bytes(data[6:8], "little") != 0
        or int.from_bytes(data[14:16], "little") != 0
        or int.from_bytes(data[36:40], "little") != 0
    ):
        return "err|invalid-data"

    total_len = int.from_bytes(data[8:12], "little")
    name_len = int.from_bytes(data[12:14], "little")
    expected_len = HEADER_LEN + name_len
    if total_len != expected_len:
        return "err|invalid-data"
    if len(data) < total_len:
        return "err|unexpected-eof"
    if len(data) != total_len:
        return "err|invalid-data"

    stored_crc = int.from_bytes(data[CRC_OFFSET : CRC_OFFSET + 4], "little")
    if stored_crc != record_crc(data):
        return "err|invalid-data"

    parent = int.from_bytes(data[16:24], "little")
    target = int.from_bytes(data[24:32], "little")
    if parent == 0 or target == 0:
        return "err|invalid-data"

    raw_name = data[HEADER_LEN:]
    try:
        name = raw_name.decode("utf-8")
    except UnicodeDecodeError:
        return "err|invalid-data"
    if (
        not name
        or name in {".", ".."}
        or "/" in name
        or "\x00" in name
        or len(raw_name) > NAME_MAX
    ):
        return "err|invalid-data"

    return f"ok|{parent}|{target}|{raw_name.hex()}"


def main() -> None:
    sys.stdout.write(canonical(sys.stdin.buffer.read()) + "\n")


if __name__ == "__main__":
    main()
