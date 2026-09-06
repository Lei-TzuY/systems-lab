# TFTP Parser Conformance Integration

This integration is a bounded executable contract between two independently imported `systems-lab` projects:

- `systems-conformance-lab` supplies real-process differential execution plus deterministic byte-mutation scheduling;
- `userspace-tcpip-stack` supplies the production `toy_tcpip::tftp::TftpPacket::parse` implementation.

The Rust adapter links directly to the imported `toy_tcpip` crate, reads one packet from standard input, executes the production parser, and emits a canonical semantic result. `oracle.py` is an independent Python parser for the same deliberately narrow TFTP packet grammar. `test_conformance.py` compares both real processes over a reviewed corpus of valid and malformed packets, then exhausts deterministic single-bit mutations of representative RRQ, ACK, and ERROR seeds through `DeterministicByteMutations` and `run_fuzz_campaign`.

## Verified boundary

A green integration gate proves only that the imported TFTP parser and the independent oracle agree on the exercised parser semantics: RRQ/WRQ C-string framing and mode handling, DATA block-size bounds, ACK exact length, ERROR message framing, opcode handling, UTF-8 validation, trailing-data rejection, and the deterministic single-bit mutation schedule.

It does **not** claim whole-stack RFC conformance, UDP/socket interoperability, a working TFTP transfer or server, security hardening, performance equivalence, MinIOS networking, container networking, or correctness of protocols outside this TFTP parser boundary.

## Run locally

```bash
python -m pip install -e 'projects/systems-conformance-lab[dev]'
cargo build --manifest-path integrations/tftp-conformance/Cargo.toml
python -m pytest -q integrations/tftp-conformance/test_conformance.py
```
