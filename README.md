# Systems Lab

A portfolio-oriented umbrella for low-level systems projects spanning virtualization, operating systems, filesystems, networking, container/process isolation, and cross-cutting correctness infrastructure.

The goal is **not** to dump similarly themed repositories into one directory or pretend that every project already composes into one operating system. Projects enter this umbrella through history-preserving migration, source-equivalent CI, and explicit integration contracts. An edge is shown as verified only when an executable regression proves it.

## Phase 0 checkpoint

Five source histories are now independently imported and verified:

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

## Architectural map

```text
                    cross-cutting correctness
                systems-conformance-lab
                         │ adapters only
                         │ future verified edges
                         ▼
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

Every arrow labeled `future` is an architecture hypothesis, **not** a current interoperability claim. `systems-conformance-lab` is target-independent infrastructure; importing it does not imply that any target already has an adapter. `mini-container-runtime` remains a Linux-host lane rather than being falsely placed inside `minios-x86`.

## Verified migration evidence

- `filesystem-lab`: source `1414e9fc4646b6c482d23d0741a0e420e8fd396c`; subtree `8b2d286e864edbdbd22d9add82c025a9dddb9604`; PR #3 and exact merged-main format/Clippy/test gates passed.
- `mini-hypervisor`: source `d32685b5453c3d1ae86ff76d0beac2b4af47094f`; subtree `709291040efb288315d3d81e26b6f4e2dfe5760b`; PR #6 and exact merged-main format/Clippy/test/build/rustdoc/MSRV/strict-KVM gates passed.
- `minios-x86`: source `e63d4218ea91069506b05944ead5a9198bf8568a`; subtree `56fec38f4af154e8c8dd7a993dcf70327c4ad7d0`; PR #9 and exact merged-main build/static/QEMU/ASan/UBSan/stress-mutant gates passed.
- `systems-conformance-lab`: source `b7df22b7004838b55054ec3d8d7b7a3b34df8137`; subtree `3ba8a9c3adbddb39121b82691a93573286d555e3`; PR #11 and exact merged-main ancestry/tree plus Ubuntu/macOS/Windows × Python 3.11/3.13 `pytest` + `ruff` matrix passed.
- `userspace-tcpip-stack`: source `2e4a58c027a18a4c3dc1d466d3adbe8b13550a0d`; subtree `24b537605193adf21b20849255eff3279ae26f7a`; PR #14 and exact merged-main ancestry/tree, rustfmt, Clippy, Ubuntu/macOS/Windows all-target tests + doctests + release builds, and Rust 1.88 MSRV gates passed.

`projects/manifest.json` is a machine-checked evidence ledger. Pull requests and `main` run `scripts/validate_manifest.py`, which rejects malformed freezes, HOLD entries without blockers, READY entries without successful source-CI evidence, duplicate project identities, and verified imports whose target subtree is missing.

## What would count as real integration?

Examples of acceptable future edges include:

- booting a deterministic `minios-x86` artifact under `mini-hypervisor` and asserting a machine-observable guest milestone;
- mounting or replaying a shared, explicitly specified filesystem image through both an OS-side reader and `filesystem-lab`;
- driving `userspace-tcpip-stack` through a concrete TAP/TUN, namespace, packet-fixture, device, or driver boundary owned by another component;
- using a `systems-conformance-lab` adapter to compare/fuzz/fault-inject an actual target boundary and retaining the regression permanently;
- connecting `mini-container-runtime` network namespace hooks to a bounded networking fixture without pretending to replace Linux kernel networking.

Matching vocabulary, a README arrow, or merely living in the same umbrella is not integration evidence.

## Migration invariants

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

See [ROADMAP.md](ROADMAP.md) for the consolidation sequence and [docs/MIGRATION.md](docs/MIGRATION.md) for the durable evidence ledger.
