# Systems Lab

A portfolio-oriented umbrella for low-level systems projects spanning virtualization, operating systems, filesystems, networking, container/process isolation, and cross-cutting correctness infrastructure.

The goal is **not** to dump similarly themed repositories into one directory or pretend that every project already composes into one operating system. Projects enter this umbrella through history-preserving migration, source-equivalent CI, and explicit integration contracts. An edge is shown as verified only when an executable regression proves it.

## Five-import checkpoint

Five source histories are independently imported and verified:

| Project | Systems layer | Status |
| --- | --- | --- |
| [mini-hypervisor](https://github.com/Lei-TzuY/mini-hypervisor) | virtualization / VM execution | **IMPORTED / VERIFIED** — source `d32685b5...`, subtree `70929104...` |
| [minios-x86](https://github.com/Lei-TzuY/minios-x86) | x86 operating-system kernel | **IMPORTED / VERIFIED** — source `e63d4218...`, subtree `56fec38f...` |
| [filesystem-lab](https://github.com/Lei-TzuY/filesystem-lab) | filesystem / storage semantics | **IMPORTED / VERIFIED** — source `1414e9fc...`, subtree `8b2d286e...` |
| [systems-conformance-lab](https://github.com/Lei-TzuY/systems-conformance-lab) | differential/fuzz/fault/repro correctness substrate | **IMPORTED / VERIFIED** — source `b7df22b7...`, subtree `3ba8a9c3...` |
| [userspace-tcpip-stack](https://github.com/Lei-TzuY/userspace-tcpip-stack) | userspace networking / protocols | **IMPORTED / VERIFIED** — source `2e4a58c0...`, subtree `24b53760...` |
| [mini-container-runtime](https://github.com/Lei-TzuY/mini-container-runtime) | Linux namespaces/cgroups/process lifecycle | **HOLD** — draft implementation PR #392 active |

Each verified import is a non-squashed subtree migration whose exact source commit remains reachable as Git ancestry and whose imported subtree was checked for exact tree equality. Permanent migration workflows are read-only and repeat source-equivalent validation on pull requests and exact merged main.

No source repository is deleted as part of consolidation.

## Verified cross-project integrations

The umbrella now has **two independently verified executable edges**. They intentionally exercise different kinds of composition rather than multiplying cosmetic arrows.

### 1. `systems-conformance-lab` → `userspace-tcpip-stack`

The bounded contract is **TFTP parser differential conformance**. `integrations/tftp-conformance/` links a Rust process adapter directly to the imported `toy_tcpip::tftp::TftpPacket::parse` implementation. An independent Python oracle parses the same bytes. `systems-conformance-lab` drives both as real subprocess targets through `DifferentialHarness`, first across a reviewed valid/malformed corpus and then across the complete deterministic single-bit mutation schedule for representative RRQ, ACK, and ERROR seeds.

Evidence:

- integration PR #16 exact head `f2dc8bae2e77dfde0d14b428ac3e782717b3a264` passed umbrella manifest `34029446301` and TFTP conformance `34029446281`;
- PR #16 was normal-merged as `a38b878a28b80c08e2e034210dcbf1377b578df0`;
- exact merged main passed umbrella manifest `34029528608` and TFTP conformance `34029528620`.

This is not a whole-stack networking claim. It does not prove UDP/socket transport, TFTP file transfer/server behavior, MinIOS networking, container networking, security, performance, or protocols outside the exercised parser boundary.

### 2. `mini-hypervisor` → `minios-x86`

The bounded contract is **real-KVM early MinIOS kernel boot through the first intentionally unsupported legacy PIC access**. `integrations/hypervisor-minios-boot/` builds the real imported MinIOS ELF32 `kernel.bin`, parses and loads its `PT_LOAD` segments into guest RAM, installs a minimal Multiboot v1 memory-info structure, and starts vCPU0 through the imported mini-hypervisor public real-mode API. A guest-owned bridge installs a flat GDT, switches 16-bit real mode to 32-bit protected mode, sets `EAX=0x2BADB002` and `EBX` to the Multiboot info, then jumps to the real MinIOS ELF entry.

The real KVM execution must emit the exact debug-port banner `Booting Advanced OS...\n` and then stop at the exact first unsupported MinIOS PIC-remap boundary: `OUT 0x20`, size 1, count 1. That proves the ELF loader, real→protected transition, Multiboot handoff, `_start`, stack setup, `kernel_main`, VGA initialization and early kernel control flow actually executed under KVM.

Evidence:

- integration PR #18 exact head `b5299b6a06fe1d4ee0a89bb03b9c91128c2c3d98` passed umbrella manifest `34035518088` and Hypervisor MinIOS boot integration `34035518070`;
- PR #18 was normal-merged as `0f33fc597b4930e3586d9ba32c636c20e3c9c0b3`;
- exact merged main passed umbrella manifest `34035659029` and Hypervisor MinIOS boot integration `34035659001`;
- exact merged-main real-KVM output reported ELF32 entry `0x10000c`, 3 `PT_LOAD` segments, Multiboot magic `0x2badb002`, the exact boot banner, and the exact `OUT 0x20` boundary.

This is **not** a full MinIOS boot. It does not claim PIC/PIT/keyboard/ATA emulation, interrupts, shell/userspace execution, filesystem/network interoperability, security, or performance.

`integrations/manifest.json` is the machine-checked edge ledger. `scripts/validate_integrations.py` requires each verified edge to name at least two verified imported participants, pin their imported source SHAs, point to an existing integration path and workflow, and retain PR plus exact merged-main evidence and explicit scope/limitations.

## Architectural map

```text
                    cross-cutting correctness
                systems-conformance-lab
                         │
                         │ VERIFIED, narrow
                         │ TFTP parser differential contract
                         ▼
                userspace-tcpip-stack

                   virtualization layer
                    mini-hypervisor
                          │
                          │ VERIFIED, bounded real-KVM
                          │ ELF32 + Multiboot early boot
                          ▼
                     minios-x86
                 kernel / OS services
                    ╱           ╲
                   ╱             ╲
      future filesystem edge   future network edge
                ╱                 ╲
      filesystem-lab       userspace-tcpip-stack

Linux-host systems lane
        │
        ├── mini-container-runtime
        │      namespaces / cgroups / process lifecycle
        │
        └── userspace-tcpip-stack
               host-facing protocol/data-plane experiments
```

Every arrow labeled `future` is an architecture hypothesis, **not** a current interoperability claim. The two arrows labeled `VERIFIED` have permanent executable workflows and exact PR plus merged-main evidence. `mini-container-runtime` remains a Linux-host lane rather than being falsely placed inside `minios-x86`.

## Verified migration evidence

- `filesystem-lab`: source `1414e9fc4646b6c482d23d0741a0e420e8fd396c`; subtree `8b2d286e864edbdbd22d9add82c025a9dddb9604`; PR #3 and exact merged-main format/Clippy/test gates passed.
- `mini-hypervisor`: source `d32685b5453c3d1ae86ff76d0beac2b4af47094f`; subtree `709291040efb288315d3d81e26b6f4e2dfe5760b`; PR #6 and exact merged-main format/Clippy/test/build/rustdoc/MSRV/strict-KVM gates passed.
- `minios-x86`: source `e63d4218ea91069506b05944ead5a9198bf8568a`; subtree `56fec38f4af154e8c8dd7a993dcf70327c4ad7d0`; PR #9 and exact merged-main build/static/QEMU/ASan/UBSan/stress-mutant gates passed.
- `systems-conformance-lab`: source `b7df22b7004838b55054ec3d8d7b7a3b34df8137`; subtree `3ba8a9c3adbddb39121b82691a93573286d555e3`; PR #11 and exact merged-main ancestry/tree plus Ubuntu/macOS/Windows × Python 3.11/3.13 `pytest` + `ruff` matrix passed.
- `userspace-tcpip-stack`: source `2e4a58c027a18a4c3dc1d466d3adbe8b13550a0d`; subtree `24b537605193adf21b20849255eff3279ae26f7a`; PR #14 and exact merged-main ancestry/tree, rustfmt, Clippy, Ubuntu/macOS/Windows all-target tests + doctests + release builds, and Rust 1.88 MSRV gates passed.

`projects/manifest.json` remains the machine-checked import evidence ledger. Pull requests and `main` validate both the project-import ledger and the independent integration ledger.

## Flagship checkpoint

The original Phase 4 flagship criteria remain met and the architecture has moved beyond the minimum: five histories are preserved with exact merged-main CI and there are now **two non-trivial verified cross-project edges**, one correctness/protocol edge and one virtualization/OS real-KVM edge.

That makes this a stronger **Systems flagship checkpoint**, not the end state of the repository. Deeper work still includes extending the VM/OS contract beyond the first unsupported PIC access, defining a real filesystem artifact contract, integrating networking through a concrete device/packet boundary, and importing `mini-container-runtime` only after its source lane becomes stable.

## Migration and integration invariants

1. Re-check exact source `main`, open implementation PRs, recent commits and CI immediately before every import.
2. Do not import a moving implementation branch merely to make progress look faster.
3. Preserve source history with non-squashed subtree migration; never substitute ZIP/current-tree copying for history.
4. Audit newly reachable history for configured attribution metadata before import; genuine authorship is not silently rewritten.
5. Require source tree ↔ imported subtree equivalence at the frozen SHA.
6. Mirror source-equivalent native CI from the umbrella path before merge.
7. Merge migration PRs with normal merge commits so imported source ancestry remains reachable.
8. Keep original repositories available while their issues, PRs, releases and links remain useful.
9. Do not add AI/bot attribution trailers to new umbrella commits.
10. Never claim cross-project interoperability that an executable integration test does not prove.
11. Record every verified cross-project edge with exact participants, scope, limitations and PR plus merged-main CI evidence.

See [ROADMAP.md](ROADMAP.md) for the consolidation sequence and [docs/MIGRATION.md](docs/MIGRATION.md) for the durable evidence ledger.
