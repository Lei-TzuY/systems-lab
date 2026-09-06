from __future__ import annotations

import sys
from pathlib import Path

import pytest

from systems_conformance import (
    CommandTarget,
    DeterministicByteMutations,
    DifferentialHarness,
    run_fuzz_campaign,
)

ROOT = Path(__file__).resolve().parents[2]
INTEGRATION = ROOT / "integrations" / "tftp-conformance"
CANDIDATE = INTEGRATION / "target" / "debug" / "tftp-conformance-adapter"
ORACLE = INTEGRATION / "oracle.py"


def make_harness() -> DifferentialHarness:
    return DifferentialHarness(
        candidate=CommandTarget((str(CANDIDATE),)),
        oracle=CommandTarget((sys.executable, str(ORACLE))),
        timeout_seconds=5,
    )


CASES = [
    pytest.param(b"\x00\x04\x00\x01", id="ack-block-1"),
    pytest.param(b"\x00\x04\xff\xff", id="ack-max-block"),
    pytest.param(b"\x00\x03\x00\x02", id="data-empty"),
    pytest.param(b"\x00\x03\x00\x02payload", id="data-small"),
    pytest.param(b"\x00\x03\x00\x07" + b"x" * 512, id="data-max-block"),
    pytest.param(b"\x00\x01boot.bin\x00octet\x00", id="rrq-octet"),
    pytest.param(b"\x00\x02firmware.img\x00OcTeT\x00", id="wrq-mode-case-fold"),
    pytest.param(b"\x00\x01readme.txt\x00netascii\x00", id="rrq-netascii"),
    pytest.param(b"\x00\x02mailbox\x00mail\x00", id="wrq-mail"),
    pytest.param(b"\x00\x05\x00\x01not found\x00", id="error-message"),
    pytest.param(b"\x00\x05\x00\x00\x00", id="error-empty-message"),
    pytest.param(b"", id="empty"),
    pytest.param(b"\x00\x04\x00", id="too-short"),
    pytest.param(b"\x00\x09\x00\x00", id="unknown-opcode"),
    pytest.param(b"\x00\x04\x00\x01x", id="ack-trailing"),
    pytest.param(b"\x00\x03\x00\x01" + b"x" * 513, id="data-too-large"),
    pytest.param(b"\x00\x01\x00octet\x00", id="rrq-empty-filename"),
    pytest.param(b"\x00\x01boot.bin\x00octet", id="rrq-missing-mode-null"),
    pytest.param(b"\x00\x01boot.bin\x00binary\x00", id="rrq-invalid-mode"),
    pytest.param(b"\x00\x01boot.bin\x00octet\x00x", id="rrq-trailing"),
    pytest.param(b"\x00\x01\xff\x00octet\x00", id="rrq-invalid-filename-utf8"),
    pytest.param(b"\x00\x01boot.bin\x00\xff\x00", id="rrq-invalid-mode-utf8"),
    pytest.param(b"\x00\x05\x00\x01missing null", id="error-missing-null"),
    pytest.param(b"\x00\x05\x00\x01bad\x00x", id="error-trailing"),
    pytest.param(b"\x00\x05\x00\x01\xff\x00", id="error-invalid-utf8"),
]


@pytest.mark.parametrize("case", CASES)
def test_tftp_parser_matches_independent_oracle(case: bytes) -> None:
    result = make_harness().evaluate(case)

    assert result.comparison.classification == "match"
    assert result.comparison.equivalent is True
    assert result.signature is None


def test_deterministic_bit_mutations_preserve_tftp_parser_conformance() -> None:
    harness = make_harness()
    mutations = DeterministicByteMutations(
        [
            b"\x00\x01boot.bin\x00octet\x00",
            b"\x00\x04\x00\x01",
            b"\x00\x05\x00\x01not found\x00",
        ]
    )

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
