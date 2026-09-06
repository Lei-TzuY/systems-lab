from __future__ import annotations

import sys

TFTP_BLOCK_SIZE = 512
VALID_MODES = {b"netascii", b"octet", b"mail"}


def _utf8(data: bytes, field: str) -> str | None:
    try:
        data.decode("utf-8")
    except UnicodeDecodeError:
        return f"err|invalid-utf8|{field}"
    return None


def canonical(data: bytes) -> str:
    if len(data) < 4:
        return f"err|packet-too-short|{len(data)}"

    opcode = int.from_bytes(data[:2], "big")

    if opcode in (1, 2):
        rest = data[2:]
        try:
            null1 = rest.index(0)
        except ValueError:
            return "err|missing-null"

        filename = rest[:null1]
        invalid = _utf8(filename, "filename")
        if invalid is not None:
            return invalid
        if not filename:
            return "err|empty-field|filename"

        mode_rest = rest[null1 + 1 :]
        try:
            null2 = mode_rest.index(0)
        except ValueError:
            return "err|missing-null"

        mode = mode_rest[:null2]
        invalid = _utf8(mode, "mode")
        if invalid is not None:
            return invalid
        if mode.lower() not in VALID_MODES:
            return f"err|invalid-mode|{mode.hex()}"

        consumed = null1 + 1 + null2 + 1
        if consumed != len(rest):
            return f"err|trailing-data|{opcode}|{len(rest) - consumed}"

        kind = "rrq" if opcode == 1 else "wrq"
        return f"ok|{kind}|{filename.hex()}|{mode.hex()}"

    if opcode == 3:
        block = int.from_bytes(data[2:4], "big")
        payload = data[4:]
        if len(payload) > TFTP_BLOCK_SIZE:
            return f"err|data-too-large|{len(payload)}"
        return f"ok|data|{block}|{payload.hex()}"

    if opcode == 4:
        if len(data) != 4:
            return f"err|invalid-length|{opcode}|{len(data)}"
        block = int.from_bytes(data[2:4], "big")
        return f"ok|ack|{block}"

    if opcode == 5:
        code = int.from_bytes(data[2:4], "big")
        message_bytes = data[4:]
        try:
            nul = message_bytes.index(0)
        except ValueError:
            return "err|missing-null"
        if nul + 1 != len(message_bytes):
            return f"err|trailing-data|{opcode}|{len(message_bytes) - nul - 1}"
        message = message_bytes[:nul]
        invalid = _utf8(message, "error-message")
        if invalid is not None:
            return invalid
        return f"ok|error|{code}|{message.hex()}"

    return f"err|invalid-opcode|{opcode}"


def main() -> None:
    sys.stdout.write(canonical(sys.stdin.buffer.read()) + "\n")


if __name__ == "__main__":
    main()
