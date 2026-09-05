# Systems Lab

A portfolio-oriented umbrella for low-level systems projects spanning virtualization, operating systems, filesystems, networking, and container/process isolation.

The goal is **not** to dump similarly themed repositories into one directory or to pretend that every project already composes into one operating system. Projects enter this umbrella through history-preserving migration, source-equivalent CI, and explicit integration contracts. An edge is shown as verified only when an executable regression proves it.

## Candidate project map

| Project | Systems layer | Live Phase 0 status |
| --- | --- | --- |
| [mini-hypervisor](https://github.com/Lei-TzuY/mini-hypervisor) | virtualization / VM execution | HOLD — implementation PR #94 active; observed main `78ce397e...` |
| [minios-x86](https://github.com/Lei-TzuY/minios-x86) | x86 operating-system kernel | HOLD — implementation PR #35 active; observed main `0276b532...` |
| [filesystem-lab](https://github.com/Lei-TzuY/filesystem-lab) | filesystem / storage semantics | **READY FOR IMPORT** — `1414e9fc...`, no open PR, exact main CI green |
| [userspace-tcpip-stack](https://github.com/Lei-TzuY/userspace-tcpip-stack) | userspace networking / protocols | HOLD — implementation PR #330 active; observed main `34782067...` |
| [mini-container-runtime](https://github.com/Lei-TzuY/mini-container-runtime) | Linux namespaces/cgroups/process lifecycle | HOLD — implementation PR #392 active; observed main `b660e8d1...` |

The first selected import candidate is `filesystem-lab`. Selection is deliberately separate from import: it does not become `IMPORTED / VERIFIED` until a real non-squashed Git migration retains the source commit as umbrella ancestry, the imported subtree matches the frozen source, and source-equivalent CI passes from the umbrella path.

`systems-conformance-lab` is also a plausible cross-cutting systems tool, but it is deferred until the first core import proves the migration machinery. Components already consolidated into `compiler-runtime-lab` are not duplicated here simply because they touch low-level runtime topics.

No source repository is deleted as part of consolidation.

## Architectural map

```text
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

The arrows labeled `future` are **not current interoperability claims**. `mini-container-runtime` is intentionally shown as a Linux-host lane rather than falsely placed inside `minios-x86`: a container runtime depends on Linux kernel primitives, while `minios-x86` is itself a kernel project.

## What would count as real integration?

Examples of acceptable future edges include:

- booting a deterministic `minios-x86` image under `mini-hypervisor` and checking a machine-observable guest milestone;
- mounting or replaying a shared, explicitly specified filesystem image/format through both a filesystem laboratory tool and an OS-side reader;
- driving `userspace-tcpip-stack` through a real TAP/TUN, namespace, or packet-fixture boundary owned by another systems component;
- connecting `mini-container-runtime` network-namespace hooks to a bounded userspace networking fixture without pretending to replace the host kernel stack;
- sharing low-level executable/image artifacts only where the binary/ABI contract is explicit and tested.

A README arrow, matching vocabulary, or two projects both being written in Rust/C/C++ is not integration evidence.

## Phase 0 executable evidence

`projects/manifest.json` is a machine-checked migration ledger, not a decorative inventory. Pull requests and `main` run `scripts/validate_manifest.py`, which rejects duplicate project identities, malformed source freezes, HOLD entries without blockers, and READY entries without successful source-CI evidence. Future imported states additionally require the target subtree to exist.

For the selected `filesystem-lab` freeze, the source-equivalent umbrella contract is:

```text
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features
```

Those commands must run from `projects/filesystem-lab` after the actual history-preserving import. Passing the manifest validator alone is never sufficient to call a source imported.

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

See [ROADMAP.md](ROADMAP.md) for the consolidation sequence and [docs/MIGRATION.md](docs/MIGRATION.md) for the evidence ledger.
