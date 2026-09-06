# Systems Lab Roadmap

## Phase 0 — Umbrella bootstrap

- [x] create the public `systems-lab` repository;
- [x] define systems-layer boundaries and forbid fake interoperability claims;
- [x] establish machine-checked migration state and source-specific permanent gates;
- [x] import and verify `filesystem-lab` with preserved source ancestry;
- [x] import and verify `mini-hypervisor` with preserved source ancestry;
- [x] repair/freeze, import, and verify `minios-x86` with preserved source ancestry;
- [x] promote, import, and verify `systems-conformance-lab` as cross-cutting correctness infrastructure;
- [x] repair/freeze, import, and verify `userspace-tcpip-stack` with preserved source ancestry;
- [x] keep exact merged-main verification green for all five imported projects.

Phase 0 migration checkpoint contains five verified histories:

- `filesystem-lab@1414e9fc4646b6c482d23d0741a0e420e8fd396c` via subtree `8b2d286e864edbdbd22d9add82c025a9dddb9604`;
- `mini-hypervisor@d32685b5453c3d1ae86ff76d0beac2b4af47094f` via subtree `709291040efb288315d3d81e26b6f4e2dfe5760b`;
- `minios-x86@e63d4218ea91069506b05944ead5a9198bf8568a` via subtree `56fec38f4af154e8c8dd7a993dcf70327c4ad7d0`;
- `systems-conformance-lab@b7df22b7004838b55054ec3d8d7b7a3b34df8137` via subtree `3ba8a9c3adbddb39121b82691a93573286d555e3`;
- `userspace-tcpip-stack@2e4a58c027a18a4c3dc1d466d3adbe8b13550a0d` via subtree `24b537605193adf21b20849255eff3279ae26f7a`.

All five were normal-merged only after exact candidate verification and then re-verified on exact merged main.

## Phase 1 — Source preflight / remaining imports

Every candidate is rechecked immediately before migration for exact `main`, open implementation PRs, exact-head CI, reachable-history attribution, repository hygiene, and source-specific native/integration gates.

- [x] `mini-hypervisor` — IMPORTED / VERIFIED.
- [x] `minios-x86` — IMPORTED / VERIFIED after source PR #35 was repaired and exact merged-main QEMU/sanitizer/mutation evidence passed.
- [x] `filesystem-lab` — IMPORTED / VERIFIED.
- [x] `systems-conformance-lab` — IMPORTED / VERIFIED with source-native 3-OS × 2-Python test/lint matrix.
- [x] `userspace-tcpip-stack` — IMPORTED / VERIFIED after source PR #330 was repaired, source exact-main Rust/Clippy gates passed, and umbrella PR plus exact merged-main source-equivalent gates passed.
- [ ] `mini-container-runtime` — **HOLD** while draft implementation PR #392 remains active.

A moving source is deferred without blocking the checkpoint. A previous green source run never overrides a later active implementation PR or changed source head.

## Phase 2 — History-preserving import procedure

For every selected source:

1. freeze exact source and umbrella SHAs;
2. scan reachable/newly reachable history and repository hygiene;
3. perform non-squashed `git subtree add` or `git subtree pull`;
4. prove source SHA remains umbrella ancestry;
5. prove source tree equals imported subtree at the frozen point;
6. run source-equivalent native CI from the umbrella path;
7. publish a migration PR only after temporary write-capable bootstrap files are removed;
8. merge with a normal merge commit;
9. rerun exact merged-main CI.

Filesystem, hypervisor, MinIOS, conformance, and userspace networking imports are reference implementations of this procedure across Rust, C/assembly/QEMU, and cross-platform Python projects.

## Phase 3 — Executable systems integration

**First verified edge complete; broader integration continues.** Import verification is not integration verification.

- [ ] `mini-hypervisor` → `minios-x86`: boot a deterministic guest artifact and assert a bounded guest-visible milestone. The current imported hypervisor exposes real-mode fixtures and an ELF64/long-mode loader, while MinIOS is an ELF32 Multiboot protected-mode kernel; do not fake this edge without a reusable protected-mode guest contract.
- [x] `systems-conformance-lab` → `userspace-tcpip-stack`: **TFTP parser differential conformance**. A Rust adapter invokes the imported production parser, an independent Python oracle implements the bounded grammar, and `DifferentialHarness` plus deterministic mutation scheduling permanently compare both real processes. PR #16 and exact merged main both passed the dedicated integration gate.
- [ ] `minios-x86` ↔ `filesystem-lab`: define and test an explicit shared image/format or replay contract;
- [ ] `minios-x86` ↔ networking: only after a concrete packet/device/driver boundary exists;
- [ ] `mini-container-runtime` ↔ `userspace-tcpip-stack`: bounded namespace/TAP/TUN/packet-fixture integration after the container source reaches a stable checkpoint;
- [ ] additional `systems-conformance-lab` adapters against named real boundaries;
- [ ] common top-level developer entrypoints where they do not weaken project-local build systems.

The TFTP edge is intentionally narrow: it proves parser-level differential agreement for the exercised corpus and deterministic mutation schedule, not whole-stack RFC conformance or transport interoperability.

The deepest next candidate remains `mini-hypervisor` → `minios-x86`, but only an executable protected-mode/Multiboot contract can advance it.

## Phase 4 — Systems flagship checkpoint

**Complete at the first flagship checkpoint.** The originally defined criteria are now all met:

- several source histories preserved with exact merged-main CI green — **met**;
- at least one non-trivial cross-project systems edge executable and permanently tested — **met** by the TFTP parser differential contract;
- README and machine ledgers distinguish verified edges from hypotheses — **met**;
- original source repositories remain available — **met**;
- no unresolved umbrella migration PR is falsely represented as completed work — **met at this checkpoint**.

This status means the umbrella has crossed from verified consolidation into verified composition. It does **not** mean every source is imported or every architectural edge is complete: `mini-container-runtime` remains on HOLD, and deeper VM/OS/filesystem/network integrations remain future milestones.

## Phase 5 — Deeper composition

- [ ] add a reusable protected-mode guest primitive only when the `mini-hypervisor` source lane is stable, then prove a deterministic MinIOS Multiboot boot milestone;
- [ ] extend conformance adapters to additional named boundaries without turning the framework into target-specific logic;
- [ ] define a shared filesystem artifact contract before claiming MinIOS/filesystem interoperability;
- [ ] import/refresh `mini-container-runtime` only after its source implementation lane is clean, then evaluate a bounded host-network integration;
- [ ] keep every new edge independently machine-ledgered with exact PR and merged-main evidence.

## Non-goals

- building a fake monolithic OS by renaming independent projects;
- forcing all projects into one language or build system;
- claiming that a Linux container runtime runs on `minios-x86` without the required kernel primitives;
- inventing filesystem/network/VM/conformance compatibility without a shared executable boundary;
- rewriting genuine authorship to make migration history cleaner;
- importing moving source repositories just to increase the umbrella project count.
