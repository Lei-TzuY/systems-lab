# Systems Lab Roadmap

## Phase 0 — Umbrella bootstrap

- [x] create the public `systems-lab` repository;
- [x] define systems-layer boundaries and forbid fake interoperability claims;
- [x] select the first candidate set: hypervisor, OS, filesystem, networking, container runtime;
- [ ] complete live source preflight for all five candidates;
- [ ] choose the first history-preserving import from a stable source checkpoint.

## Phase 1 — Source preflight

Every candidate must be rechecked immediately before migration for exact `main`, open implementation PRs, exact-head CI, reachable-history attribution, repository hygiene, and source-specific native/integration gates.

Candidate order is determined by **stability + integration value**, not by repository age or size.

- [ ] `mini-hypervisor`
- [ ] `minios-x86`
- [ ] `filesystem-lab`
- [ ] `userspace-tcpip-stack`
- [ ] `mini-container-runtime`

A project with an active implementation PR or moving source head may be deferred without blocking Phase 0.

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
