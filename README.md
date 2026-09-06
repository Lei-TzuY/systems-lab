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

## First verified cross-project integration

`systems-conformance-lab` → `userspace-tcpip-stack` now has one deliberately narrow executable edge: **TFTP parser differential conformance**.

The integration under `integrations/tftp-conformance/` links a Rust process adapter directly to the imported `toy_tcpip::tftp::TftpPacket::parse` implementation. An independent Python oracle parses the same bytes. `systems-conformance-lab` drives both as real subprocess targets through `DifferentialHarness`, first across a reviewed valid/malformed corpus and then across the complete deterministic single-bit mutation schedule for representative RRQ, ACK, and ERROR seeds.

Evidence:

- integration PR #16 exact head `f2dc8bae2e77dfde0d14b428ac3e782717b3a264` passed umbrella manifest `34029446301` and TFTP conformance `34029446281`;
- PR #16 was normal-merged as `a38b878a28b80c08e2e034210dcbf1377b578df0`;
- exact merged main passed umbrella manifest `34029528608` and TFTP conformance `34029528620`;
- the permanent integration workflow is read-only and retriggers when the integration, TCP/IP subtree, or conformance subtree changes.

This is **not** a whole-stack networking claim. It does not prove UDP/socket transport, TFTP file transfer/server behavior, MinIOS networking, container networking, security, performance, or protocols outside the exercised TFTP parser boundary.

`integrations/manifest.json` is the machine-checked edge ledger. `scripts/validate_integrations.py` requires each verified edge to name at least two verified imported participants, pin their imported source SHAs, point to an existing integration path and workflow, and retain PR plus exact merged-main evidence and explicit scope/limitations.

## Architectural map

```text
                    cross-cutting correctness
                systems-conformance-lab
                    │              ╲
                    │               ╲ VERIFIED, narrow
                    │                ╲ TFTP parser differential contract
                    │                 ╲
                    │                  ▼
                    │          userspace-tcpip-stack
                    │
                    │ future target adapters
                    ▼
              other systems boundaries

                   virtualization layer
                    mini-hypervisor
                          │
                          │ future verified guest contract
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

Every arrow labeled `future` is an architecture hypothesis, **not** a current interoperability claim. The only currently verified cross-project edge is the explicitly bounded TFTP parser differential contract above. `mini-container-runtime` remains a Linux-host lane rather than being falsely placed inside `minios-x86`.

## Verified migration evidence

- `filesystem-lab`: source `1414e9fc4646b6c482d23d0741a0e420e8fd396c`; subtree `8b2d286e864edbdbd22d9add82c025a9dddb9604`; PR #3 and exact merged-main format/Clippy/test gates passed.
- `mini-hypervisor`: source `d32685b5453c3d1ae86ff76d0beac2b4af47094f`; subtree `709291040efb288315d3d81e26b6f4e2dfe5760b`; PR #6 and exact merged-main format/Clippy/test/build/rustdoc/MSRV/strict-KVM gates passed.
- `minios-x86`: source `e63d4218ea91069506b05944ead5a9198bf8568a`; subtree `56fec38f4af154e8c8dd7a993dcf70327c4ad7d0`; PR #9 and exact merged-main build/static/QEMU/ASan/UBSan/stress-mutant gates passed.
- `systems-conformance-lab`: source `b7df22b7004838b55054ec3d8d7b7a3b34df8137`; subtree `3ba8a9c3adbddb39121b82691a93573286d555e3`; PR #11 and exact merged-main ancestry/tree plus Ubuntu/macOS/Windows × Python 3.11/3.13 `pytest` + `ruff` matrix passed.
- `userspace-tcpip-stack`: source `2e4a58c027a18a4c3dc1d466d3adbe8b13550a0d`; subtree `24b537605193adf21b20849255eff3279ae26f7a`; PR #14 and exact merged-main ancestry/tree, rustfmt, Clippy, Ubuntu/macOS/Windows all-target tests + doctests + release builds, and Rust 1.88 MSRV gates passed.

`projects/manifest.json` remains the machine-checked import evidence ledger. Pull requests and `main` validate both the project-import ledger and the independent integration ledger.

## Flagship checkpoint

The original Phase 4 flagship criteria are now met: multiple histories are preserved with exact merged-main CI, one non-trivial cross-project edge is executable and permanently tested, verified edges are distinguished from hypotheses in both human and machine ledgers, original source repositories remain available, and no unresolved umbrella migration PR is represented as completed work.

That makes this the **first Systems flagship checkpoint**, not the end state of the repository. Deeper edges remain open work; in particular, `mini-hypervisor` → `minios-x86` still requires a real protected-mode/Multiboot guest contract rather than forcing the current ELF64/long-mode path to impersonate compatibility.

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
