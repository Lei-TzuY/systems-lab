# Systems Lab Roadmap

## Phase 0 — Umbrella bootstrap

- [x] create the public `systems-lab` repository;
- [x] define systems-layer boundaries and forbid fake interoperability claims;
- [x] select the first candidate set: hypervisor, OS, filesystem, networking, container runtime;
- [x] complete the first live source preflight across all five core candidates;
- [x] choose `filesystem-lab` as the first history-preserving import candidate;
- [x] record exact source freezes, current implementation blockers and source CI evidence in the manifest/ledger;
- [ ] execute the first non-squashed Git import and prove source ancestry/tree equivalence;
- [ ] run source-equivalent `filesystem-lab` CI from the umbrella path and merge only when the exact candidate is green.

## Phase 1 — Source preflight

Every candidate must be rechecked immediately before migration for exact `main`, open implementation PRs, exact-head CI, reachable-history attribution, repository hygiene, and source-specific native/integration gates.

Candidate order is determined by **stability + integration value**, not by repository age or size.

- [ ] `mini-hypervisor` — HOLD while PR #94 is active; observed main `78ce397e...`, main CI successful.
- [ ] `minios-x86` — HOLD while PR #35 is active; observed main `0276b532...`, observed main static-analysis gate successful.
- [x] `filesystem-lab` — READY FOR IMPORT at `1414e9fc...`; no open PR, exact main CI successful, history/hygiene preflight clean.
- [ ] `userspace-tcpip-stack` — HOLD while PR #330 is active; observed main `34782067...`, observed main Clippy gate successful.
- [ ] `mini-container-runtime` — HOLD while PR #392 is active; observed main `b660e8d1...`, observed main Tests gate successful.

A project with an active implementation PR or moving source head is deferred without blocking Phase 0. A previous green source run never overrides a later source change.

`systems-conformance-lab` is a useful secondary candidate, but it remains deferred until the umbrella has proven one core migration end to end. Projects already imported into `compiler-runtime-lab` are intentionally not duplicated here.

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

The first planned source-equivalent gate is the `filesystem-lab` native contract: format check, Clippy with warnings denied, then all-target/all-feature tests from `projects/filesystem-lab`.

## Phase 3 — Executable systems integration

Possible edges must be proven, not assumed:

- [ ] `mini-hypervisor` → `minios-x86`: boot a deterministic guest artifact and assert a bounded guest-visible milestone;
- [ ] `minios-x86` ↔ `filesystem-lab`: define an explicit shared image/format or replay contract before claiming filesystem interoperability;
- [ ] `minios-x86` ↔ networking: only after a concrete packet/device/driver boundary exists;
- [ ] `mini-container-runtime` ↔ `userspace-tcpip-stack`: explore a bounded network-namespace/TAP/TUN/packet fixture without pretending the userspace stack replaces Linux kernel networking;
- [ ] define common top-level developer entrypoints while preserving project-local build systems.

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
