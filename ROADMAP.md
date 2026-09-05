# Systems Lab Roadmap

## Phase 0 — Umbrella bootstrap

- [x] create the public `systems-lab` repository;
- [x] define systems-layer boundaries and forbid fake interoperability claims;
- [x] establish machine-checked migration state and source-specific permanent gates;
- [x] import and verify `filesystem-lab` with preserved source ancestry;
- [x] import and verify `mini-hypervisor` with preserved source ancestry;
- [x] repair/freeze, import, and verify `minios-x86` with preserved source ancestry;
- [x] promote, import, and verify `systems-conformance-lab` as cross-cutting correctness infrastructure;
- [x] keep exact merged-main verification green for all four imported projects.

Phase 0 migration checkpoint now contains four verified histories:

- `filesystem-lab@1414e9fc4646b6c482d23d0741a0e420e8fd396c` via subtree `8b2d286e864edbdbd22d9add82c025a9dddb9604`;
- `mini-hypervisor@d32685b5453c3d1ae86ff76d0beac2b4af47094f` via subtree `709291040efb288315d3d81e26b6f4e2dfe5760b`;
- `minios-x86@e63d4218ea91069506b05944ead5a9198bf8568a` via subtree `56fec38f4af154e8c8dd7a993dcf70327c4ad7d0`;
- `systems-conformance-lab@b7df22b7004838b55054ec3d8d7b7a3b34df8137` via subtree `3ba8a9c3adbddb39121b82691a93573286d555e3`.

All four were normal-merged only after exact candidate verification and then re-verified on exact merged main.

## Phase 1 — Source preflight / remaining imports

Every candidate is rechecked immediately before migration for exact `main`, open implementation PRs, exact-head CI, reachable-history attribution, repository hygiene, and source-specific native/integration gates.

- [x] `mini-hypervisor` — IMPORTED / VERIFIED.
- [x] `minios-x86` — IMPORTED / VERIFIED after source PR #35 was repaired and exact merged-main QEMU/sanitizer/mutation evidence passed.
- [x] `filesystem-lab` — IMPORTED / VERIFIED.
- [x] `systems-conformance-lab` — IMPORTED / VERIFIED with source-native 3-OS × 2-Python test/lint matrix.
- [ ] `userspace-tcpip-stack` — **HOLD** while implementation PR #330 remains active.
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

Filesystem, hypervisor, MinIOS, and conformance imports are now reference implementations of this procedure across Rust, C/assembly/QEMU, and cross-platform Python projects.

## Phase 3 — Executable systems integration

**Not complete.** Import verification is not integration verification.

Candidate edges, in likely value order:

- [ ] `mini-hypervisor` → `minios-x86`: boot a deterministic guest artifact and assert a bounded guest-visible milestone;
- [ ] `systems-conformance-lab` → a real target adapter: compare/fuzz/fault-inject a named artifact/protocol/process boundary and retain a permanent regression;
- [ ] `minios-x86` ↔ `filesystem-lab`: define and test an explicit shared image/format or replay contract;
- [ ] `minios-x86` ↔ networking: only after a concrete packet/device/driver boundary exists;
- [ ] `mini-container-runtime` ↔ `userspace-tcpip-stack`: bounded namespace/TAP/TUN/packet-fixture integration after both sources reach stable checkpoints;
- [ ] common top-level developer entrypoints where they do not weaken project-local build systems.

The highest-value next integration candidate remains `mini-hypervisor` → `minios-x86`. It must be implemented as a deterministic executable contract; merely wiring paths, copying artifacts, or drawing an architecture arrow does not count.

## Phase 4 — Systems flagship checkpoint

**Not yet complete.** A flagship checkpoint requires all of the following:

- several source histories preserved with exact merged-main CI green — **met**;
- at least one non-trivial cross-project systems edge executable and permanently tested — **not yet met**;
- README/manifest/ledger distinguish verified edges from hypotheses — **met**;
- original source repositories remain available — **met**;
- no unresolved migration PR is falsely represented as completed work — **met at this checkpoint**.

Therefore the current state is a **four-import migration checkpoint**, not a completed systems flagship.

## Non-goals

- building a fake monolithic OS by renaming independent projects;
- forcing all projects into one language or build system;
- claiming that a Linux container runtime runs on `minios-x86` without the required kernel primitives;
- inventing filesystem/network/VM/conformance compatibility without a shared executable boundary;
- rewriting genuine authorship to make migration history cleaner;
- importing moving source repositories just to increase the umbrella project count.
