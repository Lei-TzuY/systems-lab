# Systems Lab Migration Protocol & Ledger

This document is the durable preflight, migration, and cross-project integration evidence ledger for `systems-lab`.

## Status vocabulary

- **PRE-FLIGHT** — candidate selected; exact live source state still needs migration-time verification.
- **READY FOR IMPORT** — exact source head/open-PR/CI/history/hygiene gates are clean for a frozen candidate.
- **HOLD** — active implementation, CI, attribution, or external validation blocker prevents a safe import.
- **IMPORTED / VERIFIED** — non-squashed source history is retained as umbrella ancestry, selected tree matches source, and source-equivalent umbrella CI is green.
- **INTEGRATION VERIFIED** — an executable cross-project contract is permanently tested in addition to import verification.

## Candidate ledger — five-import checkpoint 2026-09-06

| Project | Layer | Frozen / observed source | Source CI evidence | Status / blocker |
| --- | --- | --- | --- | --- |
| `mini-hypervisor` | virtualization | `d32685b5453c3d1ae86ff76d0beac2b4af47094f` | CI `33972996585` + strict KVM `33972996539` | **IMPORTED / VERIFIED** |
| `minios-x86` | operating-system kernel | `e63d4218ea91069506b05944ead5a9198bf8568a` | Static `33974342423`, Tests `33974342445`, Kernel `33974342424` | **IMPORTED / VERIFIED** |
| `filesystem-lab` | filesystem/storage | `1414e9fc4646b6c482d23d0741a0e420e8fd396c` | CI `33971985113` | **IMPORTED / VERIFIED** |
| `systems-conformance-lab` | cross-cutting correctness | `b7df22b7004838b55054ec3d8d7b7a3b34df8137` | CI `33974760080` | **IMPORTED / VERIFIED** |
| `userspace-tcpip-stack` | userspace networking/protocols | `2e4a58c027a18a4c3dc1d466d3adbe8b13550a0d` | Rust CI `34024025353` + Clippy `34024025409` | **IMPORTED / VERIFIED** |
| `mini-container-runtime` | Linux container/process isolation | observed `b660e8d14aebf181e29ad844c18f7133ad0334ea` | prior source evidence successful | **HOLD** — draft implementation PR #392 active |

Every future import or refresh is rechecked immediately before execution; historical green evidence never overrides a moving head, active implementation PR, or new failure.

## Completed import 1 — `filesystem-lab`

Frozen source: `1414e9fc4646b6c482d23d0741a0e420e8fd396c`.

- bootstrap `33972535890` repeated source-head and zero-open-PR checks;
- subtree commit `8b2d286e864edbdbd22d9add82c025a9dddb9604` retains the source commit as second parent and has exact tree equality at `projects/filesystem-lab`;
- import PR #3 head `d41f34a53a18353a67225e69b2a17bc6129e4b38` passed manifest `33972592152` and verification `33972592139`;
- normal merge `16ec31643891fe6d587339f1bea543fefee2189f` then passed exact merged-main manifest `33972631312` and filesystem verification `33972631346`.

Permanent workflow is read-only. No MinIOS/filesystem interoperability is implied.

## Completed import 2 — `mini-hypervisor`

Frozen source: `d32685b5453c3d1ae86ff76d0beac2b4af47094f` after source PR #94 was normal-merged and exact source main passed both standard CI and strict real-KVM INTx proof.

- source CI `33972996585`; strict KVM `33972996539`;
- bootstrap `33973179661` repeated exact source-main and zero-open-PR checks;
- subtree commit `709291040efb288315d3d81e26b6f4e2dfe5760b` retains exact source as second parent and exact tree identity;
- final import PR #6 head `238a8695f00515ad1540fa847fbbedab880776db` passed manifest `33973305830` and verification `33973305829`;
- normal merge `f436b8863a688fcc34300577263d7dab7d00407f` passed exact merged-main manifest `33973353798` and hypervisor verification `33973353484`.

The permanent gate covers format, Clippy, tests, build, rustdoc, the source Rust 1.74 shipped-target contract, and strict real-KVM virtio-blk INTx proof. Import verification alone did not claim hypervisor → MinIOS compatibility; that claim is separately bounded by Verified integration 2 below.

## Completed import 3 — `minios-x86`

Frozen source: `e63d4218ea91069506b05944ead5a9198bf8568a`.

Before freezing, source PR #35 was repaired rather than imported around a known correctness failure. The final design uses a dedicated per-process `waitpid_event` to wake sibling `waitpid()` callers without broadly waking unrelated waiters sharing a process channel. Exact PR-head Static, Tests, Kernel/QEMU, ASan/UBSan and mutation evidence passed before source normal merge; exact source merged-main runs then passed again.

### Pre-import evidence

- source Static `33974342423` success;
- source Tests `33974342445` success;
- source Kernel `33974342424` success, including native/QEMU regressions, host ASan/UBSan and stress-mutant kill proof;
- zero open source PRs at final freeze and migration-time bootstrap;
- configured attribution searches returned no configured disallowed matches;
- no `.gitmodules` declaration or obvious committed build/cache payload observed.

### History and umbrella evidence

- bootstrap `33975093404` rechecked source `main` and zero open PRs;
- subtree commit `56fec38f4af154e8c8dd7a993dcf70327c4ad7d0` has exact source `e63d4218...` as second parent and exact tree equality at `projects/minios-x86`;
- import PR #9 head `3a6bf88399eef68e438710c26b94658a753b9035` passed manifest `33975138166` and MinIOS verification `33975138165`;
- PR #9 normal-merged as `2ba4b6cb2b394456df5319e7b9c980ae493aace2`;
- exact merged main passed manifest `33975669912` and MinIOS verification `33975669767`, repeating build, static analysis, QEMU/native regression, ASan/UBSan, and stress-mutant proof.

The temporary write-capable bootstrap workflow and trigger were removed before the import PR. The permanent MinIOS workflow is read-only. Import verification itself does not imply VM/filesystem/network interoperability.

## Completed import 4 — `systems-conformance-lab`

Frozen source: `b7df22b7004838b55054ec3d8d7b7a3b34df8137`.

The source is treated as target-independent correctness infrastructure: differential execution, fuzzing, fault injection, reduction, deterministic reproduction and target adapters. Importing the framework does not imply any target adapter already exists.

### Pre-import evidence

- exact source-main CI `33974760080` success across Ubuntu/macOS/Windows × Python 3.11/3.13;
- every matrix cell ran editable development install, `python -m pytest`, and `python -m ruff check .` successfully;
- zero open source PRs at freeze and again at bootstrap;
- configured attribution searches returned no configured disallowed matches;
- recursive tree inspection showed source/docs/tests/package metadata without `.gitmodules` or obvious committed cache/build payload.

### History and umbrella evidence

- bootstrap `33976134168` rechecked exact source main and zero open PRs, then verified tree identity;
- subtree commit `3ba8a9c3adbddb39121b82691a93573286d555e3` has exact source `b7df22b7...` as second parent and exact tree equality at `projects/systems-conformance-lab`;
- temporary write-capable bootstrap workflow and trigger were removed before the import PR;
- import PR #11 head `6ea0d5182e3432f47701197d1826b9e8b46c18d0` passed manifest `33976223333` and Conformance migration `33976223338`, including preserved ancestry/tree and all six OS/Python matrix cells;
- PR #11 normal-merged as `7f639e3ec6e12f14a370e874faad837409c0ccec`;
- exact merged main passed manifest `33976275067` and Conformance migration `33976275089`, repeating the same history/tree and six-cell source-equivalent matrix.

The permanent Conformance workflow is read-only. No conformance → target integration is implied by the import itself.

## Completed import 5 — `userspace-tcpip-stack`

Frozen source: `2e4a58c027a18a4c3dc1d466d3adbe8b13550a0d`.

The source was not frozen while PR #330 was red or active. Its checked PTP transparent-clock arithmetic change first required a shell caller repair so new fallible APIs were handled without panics or false success output. Only after source PR #330 was repaired, normal-merged, and exact merged-main Rust/Clippy gates were green was migration allowed.

### Pre-import evidence

- source Rust CI `34024025353` success: rustfmt, Ubuntu/macOS/Windows all-target tests + doctests + release builds, and Rust 1.88 MSRV tests;
- source Clippy `34024025409` success with `-D clippy::correctness`;
- zero open source PRs at freeze and again inside bootstrap;
- no `.gitmodules` declaration or obvious committed top-level build/cache payload observed;
- newly reachable commits from the prior reviewed checkpoint through `2e4a58c0...` were audited for configured disallowed attribution markers with zero matches.

### History and umbrella evidence

- bootstrap `34024737036` rechecked exact frozen source and zero open PRs, then performed the non-squashed subtree import;
- subtree commit `24b537605193adf21b20849255eff3279ae26f7a` has exact source `2e4a58c0...` as second parent;
- imported subtree tree `ff5bc2211c2574e0b4c53d8bb5189a1c471b6974` exactly equals the frozen source tree;
- temporary write-capable `bootstrap-tcp.yml` and its trigger were removed from the migration branch before the import PR;
- import PR #14 head `4b35f467b8486d8adc03d52ef931ff84df90f86c` passed manifest `34024889338` and Userspace TCP/IP migration `34024889259`; the migration run passed preserved-history/tree proof, rustfmt, Clippy, Ubuntu/macOS/Windows tests + doctests + release builds, and Rust 1.88 MSRV;
- PR #14 normal-merged as `218cd62eebc248a025107231bd959dfed34f8697`;
- exact merged main passed manifest `34025487651` and Userspace TCP/IP migration `34025487653`, repeating all seven migration jobs successfully.

The permanent TCP/IP workflow is read-only. This import proves preserved history, exact tree identity, and source-equivalent standalone correctness only. It does **not** prove a MinIOS network stack, container-network integration, TAP/TUN interoperability, or any other cross-project networking edge.

## Verified integration 1 — TFTP parser differential conformance

Participants:

- `systems-conformance-lab@b7df22b7004838b55054ec3d8d7b7a3b34df8137`;
- `userspace-tcpip-stack@2e4a58c027a18a4c3dc1d466d3adbe8b13550a0d`.

Boundary: `integrations/tftp-conformance/`.

The candidate is a Rust process adapter linked directly to the imported `toy_tcpip` crate and invoking `toy_tcpip::tftp::TftpPacket::parse`. The oracle is an independent Python implementation of the deliberately bounded parser grammar. `systems-conformance-lab` runs both through `CommandTarget`/`DifferentialHarness` as real subprocesses and uses `DeterministicByteMutations` plus `run_fuzz_campaign` to exhaust the configured representative single-bit mutation schedule.

The reviewed corpus covers valid and malformed RRQ/WRQ, DATA, ACK and ERROR packets, including C-string termination, case-insensitive transfer modes, UTF-8 failures, trailing data, exact ACK length, unknown opcodes and the 512-byte DATA limit.

### Integration evidence

- integration PR #16 final head `f2dc8bae2e77dfde0d14b428ac3e782717b3a264`;
- exact PR-head umbrella manifest `34029446301` success;
- exact PR-head TFTP conformance `34029446281` success after rustfmt, Clippy, Rust 1.88 build, ruff, reviewed differential corpus and deterministic mutation campaign;
- PR #16 normal-merged as `a38b878a28b80c08e2e034210dcbf1377b578df0`;
- exact merged-main umbrella manifest `34029528608` success;
- exact merged-main TFTP conformance `34029528620` success, repeating the executable integration contract;
- permanent `.github/workflows/tftp-conformance.yml` uses read-only repository permissions and retriggers when the integration or either participating imported subtree changes.

This is an **INTEGRATION VERIFIED** edge only for the parser semantics exercised by that contract. It does not claim whole-stack or whole-RFC TFTP conformance, UDP/socket transport, file-transfer/server behavior, MinIOS networking, container networking, security, performance, or correctness of unrelated protocols.

## Verified integration 2 — `mini-hypervisor` → MinIOS real-KVM early boot

Participants:

- `mini-hypervisor@d32685b5453c3d1ae86ff76d0beac2b4af47094f`;
- `minios-x86@e63d4218ea91069506b05944ead5a9198bf8568a`.

Boundary: `integrations/hypervisor-minios-boot/`.

This edge uses the already imported, frozen project versions; no source-subtree refresh or source modification is part of the proof. The workflow builds the real MinIOS `kernel.bin` ELF32 artifact, parses and loads its `PT_LOAD` segments into 64 MiB of registered KVM guest memory, installs a minimal Multiboot v1 memory-info structure, and writes the real ELF entry point into a bounded low-memory handoff slot. vCPU0 starts through the imported hypervisor's public real-mode API at a guest-owned bridge. The bridge installs a flat GDT, enables CR0.PE, performs a 16-bit → 32-bit far jump, sets `EAX=0x2BADB002` and `EBX` to the Multiboot-info GPA, and jumps to the real MinIOS entry.

The proof then captures MinIOS's existing port-`0xE9` debug console through the imported hypervisor `PortIoBus`. Success requires the exact banner `Booting Advanced OS...\n`, followed by the exact first unsupported hardware boundary from MinIOS `idt_install()`: one-byte `OUT 0x20` with count 1. Triple faults, wrong entries, malformed ELF loads, missing/wrong banners, other unsupported ports, or normal termination cannot satisfy the contract.

### Integration evidence

- integration PR #18 final head `b5299b6a06fe1d4ee0a89bb03b9c91128c2c3d98`;
- exact PR-head umbrella manifest `34035518088` success;
- exact PR-head Hypervisor MinIOS boot integration `34035518070` success, including real MinIOS build, bridge build, Rust 1.74 rustfmt/Clippy/build and real `/dev/kvm` execution;
- PR #18 normal-merged as `0f33fc597b4930e3586d9ba32c636c20e3c9c0b3`;
- exact merged-main umbrella manifest `34035659029` success;
- exact merged-main Hypervisor MinIOS boot integration `34035659001` success;
- exact merged-main KVM log reported MinIOS ELF32 entry `0x10000c`, 3 `PT_LOAD` segments, Multiboot magic `0x2badb002`, debug proof `Booting Advanced OS...\n`, exact boundary `OUT port=0x20 size=1 count=1`, and final `VERIFIED` marker;
- permanent `.github/workflows/hypervisor-minios-boot.yml` is read-only and retriggers when the integration or either participating imported subtree changes.

This is an **INTEGRATION VERIFIED** edge only for early MinIOS boot through the first intentionally unsupported legacy PIC I/O access. It does not claim PIC/PIT/keyboard/ATA emulation, interrupt delivery, a shell or userspace session, filesystem/network interoperability, security, performance, or a complete MinIOS boot.

`integrations/manifest.json` records both verified edges separately from project import state. `scripts/validate_integrations.py` cross-checks participant source SHAs against the import ledger, requires existing integration/workflow paths, and requires explicit scope, limitations, PR evidence and exact merged-main evidence.

## Current HOLD lane

### `mini-container-runtime`

Draft implementation PR #392 (`feat(image): wire OCI Entrypoint and Cmd into run admission`) explicitly remains in progress until public CLI semantics are complete and exact candidate CI is green. Do not freeze or import this source yet.

## Preflight gate

Immediately before each import or refresh:

1. record exact source `main` SHA and umbrella `main` SHA;
2. confirm no active implementation PR owns the source surface;
3. confirm exact source CI/checks required by that repository are successful;
4. inspect recent commits so the freeze is not taken during a moving architectural rewrite;
5. scan reachable history for configured attribution policy;
6. preserve source metadata as-is rather than cosmetically rewriting it;
7. inspect generated artifacts, caches, large binaries, secrets, platform-specific fixtures, hardware/manual requirements and nested repositories;
8. define the source-equivalent umbrella CI contract before merge.

## History-preserving procedure

Use non-squashed subtree operations. A selected source commit must remain reachable from umbrella history.

```bash
git remote add source-project https://github.com/Lei-TzuY/<project>.git
git fetch source-project --tags
git subtree add --prefix=projects/<project> source-project <frozen-sha>
```

For refreshes, use a non-squashed `git subtree pull` and separately audit newly reachable source history. A current-tree copy, archive, or squash does **not** satisfy this ledger.

## Integration evidence rule

The current checkpoint verifies **five imports and two cross-project integration edges**. Import verification and integration verification remain separate claims.

A verified edge needs a named artifact/protocol/device/process boundary, deterministic setup where practical, executable assertions, honest platform constraints, and claims no broader than the exercised contract. Each verified edge is recorded in `integrations/manifest.json` with pinned participant source SHAs, exact PR and merged-main evidence, verification contract and limitations.

The two verified edges intentionally cover distinct architectural surfaces:

1. `systems-conformance-lab` → `userspace-tcpip-stack`: TFTP parser differential conformance through real subprocess execution and deterministic mutation scheduling.
2. `mini-hypervisor` → `minios-x86`: real-KVM ELF32/Multiboot early kernel boot through the exact first unsupported PIC-remap I/O boundary.

The originally defined Phase 4 criteria remain met, so the umbrella may claim a strengthened **Systems flagship checkpoint**. This is a milestone, not a terminal state: `mini-container-runtime` remains on HOLD, the VM/OS edge deliberately stops at the PIC boundary, and filesystem/network/device integrations remain open work.

## Original repository policy

Source repositories remain available after import. Archive/read-only decisions happen only after umbrella CI and canonical links are stable. Routine consolidation never deletes original repositories.
