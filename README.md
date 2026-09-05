# Systems Lab

A portfolio-oriented umbrella for low-level systems projects spanning virtualization, operating systems, filesystems, networking, and container/process isolation.

The goal is **not** to dump similarly themed repositories into one directory or to pretend that every project already composes into one operating system. Projects enter this umbrella through history-preserving migration, source-equivalent CI, and explicit integration contracts. An edge is shown as verified only when an executable regression proves it.

## Candidate project map

| Project | Systems layer | Migration status |
| --- | --- | --- |
| [mini-hypervisor](https://github.com/Lei-TzuY/mini-hypervisor) | virtualization / VM execution | PRE-FLIGHT |
| [minios-x86](https://github.com/Lei-TzuY/minios-x86) | x86 operating-system kernel | PRE-FLIGHT |
| [filesystem-lab](https://github.com/Lei-TzuY/filesystem-lab) | filesystem / storage semantics | PRE-FLIGHT |
| [userspace-tcpip-stack](https://github.com/Lei-TzuY/userspace-tcpip-stack) | userspace networking / protocols | PRE-FLIGHT |
| [mini-container-runtime](https://github.com/Lei-TzuY/mini-container-runtime) | Linux namespaces/cgroups/process lifecycle | PRE-FLIGHT |

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
