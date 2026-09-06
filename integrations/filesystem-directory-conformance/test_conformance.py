from __future__ import annotations

import sys
import zlib
from pathlib import Path

import pytest
from systems_conformance import (
    CommandTarget,
    DeterministicByteMutations,
    DifferentialHarness,
    run_fuzz_campaign,
)

ROOT = Path(__file__).resolve().parents[2]
INTEGRATION = ROOT / "integrations" / "filesystem-directory-conformance"
CANDIDATE = INTEGRATION / "target" / "debug" / "filesystem-directory-conformance-adapter"
ORACLE = INTEGRATION / "oracle.py"
HEADER_LEN = 40
CRC_OFFSET = 32


def encode_record(parent: int, target: int, name: bytes) -> bytes:
    data = bytearray(HEADER_LEN + len(name))
    data[:4] = b"DNT1"
    data[4:6] = (1).to_bytes(2, "little")
    data[8:12] = len(data).to_bytes(4, "little")
    data[12:14] = len(name).to_bytes(2, "little")
    data[16:24] = parent.to_bytes(8, "little")
    data[24:32] = target.to_bytes(8, "little")
    data[HEADER_LEN:] = name
    data[CRC_OFFSET : CRC_OFFSET + 4] = (zlib.crc32(data) & 0xFFFFFFFF).to_bytes(4, "little")
    return bytes(data)


def with_crc(data: bytearray) -> bytes:
    data[CRC_OFFSET : CRC_OFFSET + 4] = b"\x00\x00\x00\x00"
    data[CRC_OFFSET : CRC_OFFSET + 4] = (zlib.crc32(data) & 0xFFFFFFFF).to_bytes(4, "little")
    return bytes(data)


def make_harness() -> DifferentialHarness:
    return DifferentialHarness(
        candidate=CommandTarget((str(CANDIDATE),)),
        oracle=CommandTarget((sys.executable, str(ORACLE))),
        timeout_seconds=5,
    )


VALID = encode_record(2, 7, b"hello.txt")
UNICODE = encode_record(11, 42, "資料.bin".encode())
MAX_NAME = encode_record(5, 9, b"a" * 255)

bad_reserved = bytearray(VALID)
bad_reserved[6] = 1
bad_reserved = with_crc(bad_reserved)

zero_parent = bytearray(VALID)
zero_parent[16:24] = (0).to_bytes(8, "little")
zero_parent = with_crc(zero_parent)

invalid_utf8 = bytearray(VALID)
invalid_utf8[HEADER_LEN:] = b"\xffello.txt"
invalid_utf8 = with_crc(invalid_utf8)

invalid_name = bytearray(encode_record(2, 7, b"bad/name"))
invalid_name = with_crc(invalid_name)

corrupt_crc = bytearray(VALID)
corrupt_crc[-1] ^= 0x80
corrupt_crc = bytes(corrupt_crc)

CASES = [
    pytest.param(VALID, id="valid-ascii"),
    pytest.param(UNICODE, id="valid-unicode"),
    pytest.param(MAX_NAME, id="valid-max-name"),
    pytest.param(b"", id="empty"),
    pytest.param(VALID[:39], id="torn-header"),
    pytest.param(VALID[:-1], id="torn-payload"),
    pytest.param(VALID + b"x", id="trailing-data"),
    pytest.param(b"BAD!" + VALID[4:], id="bad-magic"),
    pytest.param(bad_reserved, id="reserved-field"),
    pytest.param(zero_parent, id="zero-parent"),
    pytest.param(invalid_utf8, id="invalid-utf8"),
    pytest.param(invalid_name, id="invalid-component"),
    pytest.param(corrupt_crc, id="checksum-corruption"),
]


@pytest.mark.parametrize("case", CASES)
def test_directory_record_matches_independent_oracle(case: bytes) -> None:
    result = make_harness().evaluate(case)

    assert result.comparison.classification == "match"
    assert result.comparison.equivalent is True
    assert result.signature is None


def test_all_single_bit_mutations_preserve_directory_codec_conformance() -> None:
    harness = make_harness()
    mutations = DeterministicByteMutations([VALID, UNICODE])

    result = run_fuzz_campaign(
        cases=mutations,
        evaluate=lambda case: harness.evaluate(case).comparison,
        max_evaluations=mutations.case_count,
    )

    assert result.classification == "match"
    assert result.evaluations == mutations.case_count
    assert result.exhausted_budget is True
    assert result.failing_case is None
    assert result.comparison is None
