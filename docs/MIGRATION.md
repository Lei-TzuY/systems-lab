# Systems Lab Migration Protocol & Ledger

This document is the durable preflight and migration evidence ledger for `systems-lab`.

## Status vocabulary

- **PRE-FLIGHT** — candidate selected; exact live source state still needs migration-time verification.
- **READY FOR IMPORT** — exact source head/open-PR/CI/history/hygiene gates are clean for a frozen candidate.
- **HOLD** — active implementation, CI, attribution, or external validation blocker prevents a safe import.
- **IMPORTED / VERIFIED** — non-squashed source history is retained as umbrella ancestry, selected tree matches source, and source-equivalent umbrella CI is green.
- **INTEGRATION VERIFIED** — an executable cross-project contract is permanently tested in addition to import verification.

## Candidate ledger — checkpoint 2026-09-05

| Project | Layer | Frozen / observed source | Source CI evidence | Status / blocker |
| --- | --- | --- | --- | --- |
| `mini-hypervisor` | virtualization | `d32685b5453c3d1ae86ff76d0beac2b4af47094f` | CI `33972996585` + strict KVM `33972996539` success | **IMPORTED / VERIFIED** |
| `minios-x86` | operating-system kernel | observed `0276b5326a1fbd00d2d2de26b128704b3098e42d` | prior static-analysis evidence successful | **HOLD** — implementation PR #35 active |
| `filesystem-lab` | filesystem/storage | `1414e9fc4646b6c482d23d0741a0e420e8fd396c` | CI `33971985113` success | **IMPORTED / VERIFIED** |
| `userspace-tcpip-stack` | userspace networking/protocols | observed `347820674b71f1b8203d52366604e32b0ca3fb1d` | prior Clippy evidence successful | **HOLD** — implementation PR #330 active |
| `mini-container-runtime` | Linux container/process isolation | observed `b660e8d14aebf181e29ad844c18f7133ad0334ea` | prior Tests evidence successful | **HOLD** — implementation PR #392 active |

Every future import or refresh is rechecked immediately before execution; historical green evidence never overrides a later moving head, active implementation PR, or new failure.

## Completed import 1: `filesystem-lab`

Frozen source: `1414e9fc4646b6c482d23d0741a0e420e8fd396c`.

### Pre-import evidence

- zero open source PRs immediately before import;
- exact source-main CI `33971985113` successful;
- configured reachable-history attribution scan clean;
- source tree/hygiene recorded without inventing missing metadata;
- source-native contract fixed before migration: format, Clippy with warnings denied, and all-target/all-feature tests.

### History-preservation evidence

Bootstrap `33972535890` repeated source-head and zero-open-PR checks immediately before executing the non-squashed subtree add.

Subtree commit `8b2d286e864edbdbd22d9add82c025a9dddb9604` has:

- first parent `91247951d26fbe4ac9a229ec518343792400c40b` from `systems-lab`;
- second parent `1414e9fc4646b6c482d23d0741a0e420e8fd396c` from the source;
- an imported `projects/filesystem-lab` tree exactly equal to the frozen source tree.

Import PR #3 head `d41f34a53a18353a67225e69b2a17bc6129e4b38` passed manifest `33972592152` and project verification `33972592139`. It was normal-merged as `16ec31643891fe6d587339f1bea543fefee2189f`, whose exact merged-main manifest `33972631312` and filesystem verification `33972631346` also succeeded.

The temporary write-capable bootstrap workflow/trigger did not remain on final main. The permanent filesystem gate is read-only.

No `minios-x86` ↔ `filesystem-lab` integration is implied by import verification.

## Completed import 2: `mini-hypervisor`

Frozen source: `d32685b5453c3d1ae86ff76d0beac2b4af47094f`, created by normal-merging source PR #94 after its exact candidate was green.

### Pre-import evidence

- source PR #94 exact candidate passed its main CI and strict real-KVM INTx workflow before source merge;
- exact merged source main passed CI `33972996585` and strict real-KVM INTx `33972996539`;
- source had zero open PRs at final freeze and again at migration-time bootstrap;
- configured reachable-history attribution searches returned no configured disallowed matches;
- root hygiene inspection found source metadata/docs/Rust source/tests/workflows with no observed generated binary/cache payload or nested repository.

### History-preservation evidence

Bootstrap `33973179661` repeated exact source-main and zero-open-PR checks and then performed a non-squashed subtree add.

Subtree commit `709291040efb288315d3d81e26b6f4e2dfe5760b` has:

- first parent `237278c8fcf96cb0b4e3c2ff337d3bcdc21497d8` from `systems-lab`;
- second parent `d32685b5453c3d1ae86ff76d0beac2b4af47094f` from `mini-hypervisor`;
- an imported `projects/mini-hypervisor` tree exactly equal to the frozen source tree.

This is preserved Git ancestry, not a current-tree copy or squash.

### PR and merged-main verification

The first PR #6 verification attempt correctly failed because the umbrella MSRV command used `--all-targets`, while the source repository intentionally applies Rust 1.74 only to shipped library/binary targets; test-only code uses newer `offset_of!`. The gate was corrected to the source contract, `cargo +1.74.0 check --all-features`, without modifying imported source or weakening ancestry/tree/KVM verification.

Final import PR #6 head `238a8695f00515ad1540fa847fbbedab880776db` passed:

- manifest `33973305830`;
- hypervisor verification `33973305829`: ancestry, exact tree identity, format, Clippy, all-target/all-feature tests, build, rustdoc warnings denied, Rust 1.74 shipped-target check, and strict real-KVM virtio-blk INTx proof.

PR #6 was normal-merged as `f436b8863a688fcc34300577263d7dab7d00407f`. Exact merged main then passed:

- manifest `33973353798`;
- hypervisor verification `33973353484`, repeating the same permanent contract on the integrated commit.

The temporary write-capable hypervisor bootstrap workflow and trigger were removed before the import PR merged. The permanent `Hypervisor migration` workflow remains read-only.

No `mini-hypervisor` → `minios-x86` interoperability is claimed by this import. That edge requires a deterministic guest artifact and executable guest-visible contract.

## Secondary systems candidate

`systems-conformance-lab` remains a plausible cross-cutting conformance/integration tool and is deliberately deferred pending a later scope decision. Projects already consolidated into `compiler-runtime-lab` are not duplicated here merely because they also touch systems topics.

## Preflight gate

Immediately before each import or refresh:

1. record exact source `main` SHA and umbrella `main` SHA;
2. confirm no active implementation PR owns the same source surface;
3. confirm exact source CI/checks required by that repository are successful;
4. inspect recent commits so the freeze is not taken during a moving architectural rewrite;
5. scan reachable history for configured attribution policy;
6. preserve source metadata as-is rather than cosmetically rewriting it;
7. inspect generated artifacts, caches, large binaries, secrets, platform-specific fixtures, hardware/manual requirements and nested repositories;
8. define the source-equivalent umbrella CI contract before merge.

## History-preserving procedure

Use non-squashed subtree operations. A selected source commit must remain reachable from umbrella history.

```bash
git remote add source-project https://github.com/Lei-TzuY/<project>.git
git fetch source-project --tags
git subtree add --prefix=projects/<project> source-project <frozen-sha>
```

For refreshes, use a non-squashed `git subtree pull` and separately audit newly reachable source history.

A current-tree copy, archive, or squash migration does **not** satisfy this ledger even if the files are identical. Import status advances to **IMPORTED / VERIFIED** only after ancestry, tree equivalence, and umbrella-path CI are all proven.

## Integration evidence rule

Projects sharing a systems theme are not automatically integrated. A verified edge needs a named artifact/protocol/device/process boundary, deterministic setup where practical, executable assertions, honest platform constraints, and claims no broader than the exercised contract.

Examples: boot image ↔ hypervisor, disk image ↔ filesystem implementation, TAP/TUN packet boundary ↔ network stack, namespace lifecycle ↔ container runtime.

## Original repository policy

Source repositories remain available after import. Archive/read-only decisions happen only after umbrella CI and canonical links are stable. Routine consolidation never deletes original repositories.
