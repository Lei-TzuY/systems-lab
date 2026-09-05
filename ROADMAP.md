# Systems Lab Roadmap

## Phase 0 — Umbrella bootstrap

- [x] create the public `systems-lab` repository;
- [x] define systems-layer boundaries and forbid fake interoperability claims;
- [x] select the first candidate set: hypervisor, OS, filesystem, networking, container runtime;
- [x] complete the first live source preflight across all five core candidates;
- [x] import and verify `filesystem-lab` with preserved source ancestry;
- [x] import and verify `mini-hypervisor` with preserved source ancestry;
- [x] keep executable manifest and project-specific migration gates on exact merged main.

Phase 0 now has two verified core imports:

- `filesystem-lab@1414e9fc4646b6c482d23d0741a0e420e8fd396c` through subtree commit `8b2d286e864edbdbd22d9add82c025a9dddb9604`;
- `mini-hypervisor@d32685b5453c3d1ae86ff76d0beac2b4af47094f` through subtree commit `709291040efb288315d3d81e26b6f4e2dfe5760b`.

Both imports were normal-merged only after exact PR verification and then re-verified on exact merged main.

## Phase 1 — Source preflight

Every candidate must be rechecked immediately before migration for exact `main`, open implementation PRs, exact-head CI, reachable-history attribution, repository hygiene, and source-specific native/integration gates.

Candidate order is determined by **stability + integration value**, not by repository age or size.

- [x] `mini-hypervisor` — IMPORTED / VERIFIED from `d32685b5...`; PR #6 and exact merged-main ancestry/tree/native/MSRV/real-KVM verification successful.
- [ ] `minios-x86` — HOLD while PR #35 is active; do not freeze while concurrent child-wait correctness work remains in flight.
- [x] `filesystem-lab` — IMPORTED / VERIFIED from `1414e9fc...`; PR #3 and exact merged-main ancestry/tree/native CI successful.
- [ ] `userspace-tcpip-stack` — HOLD while PR #330 is active.
- [ ] `mini-container-runtime` — HOLD while PR #392 is active.

A project with an active implementation PR or moving source head is deferred without blocking progress. A previous green source run never overrides a later source change.

`systems-conformance-lab` remains a secondary candidate. Projects already imported into `compiler-runtime-lab` are intentionally not duplicated here.

## Phase 2 — History-preserving imports

For each selected source:

1. freeze exact source and umbrella SHAs;
2. scan reachable history / newly reachable refresh history;
3. non-squashed `git subtree add` or `git subtree pull`;
4. verify source SHA remains umbrella ancestry;
5. verify source tree equals imported subtree at the frozen point;
6. run source-equivalent native CI from the umbrella path;
7. publish a migration PR;
8. merge with a normal merge commit;
9. rerun exact merged-main CI.

`filesystem-lab` and `mini-hypervisor` are now reference implementations of this procedure. The hypervisor import additionally demonstrates that source-specific hardware execution gates can remain explicit rather than being replaced by generic build success.

## Phase 3 — Executable systems integration

Import verification is not integration verification. Possible edges must be proven, not assumed:

- [ ] `mini-hypervisor` → `minios-x86`: boot a deterministic guest artifact and assert a bounded guest-visible milestone;
- [ ] `minios-x86` ↔ `filesystem-lab`: define an explicit shared image/format or replay contract before claiming filesystem interoperability;
- [ ] `minios-x86` ↔ networking: only after a concrete packet/device/driver boundary exists;
- [ ] `mini-container-runtime` ↔ `userspace-tcpip-stack`: explore a bounded network-namespace/TAP/TUN/packet fixture without pretending the userspace stack replaces Linux kernel networking;
- [ ] define common top-level developer entrypoints while preserving project-local build systems.

The highest-value next integration edge is likely `mini-hypervisor` → `minios-x86`, but it must wait until the `minios-x86` implementation lane reaches a clean frozen checkpoint. Until then, do not fabricate a guest contract from unrelated artifacts.

## Phase 4 — Systems flagship checkpoint

A checkpoint is reached only when:

- several source histories are preserved and exact merged-main CI is green;
- at least one non-trivial cross-project systems edge is executable and permanently tested;
- README/manifest/ledger distinguish verified edges from architecture hypotheses;
- original source repositories remain available;
- there are no unresolved migration PRs pretending to be finished work.

## Non-goals

- building a fake monolithic OS by renaming independent projects;
- forcing all projects into one language or build system;
- claiming that a Linux container runtime runs on `minios-x86` without the required kernel primitives;
- inventing filesystem/network/VM compatibility without a shared artifact or protocol contract;
- rewriting genuine authorship to make migration history cleaner.
