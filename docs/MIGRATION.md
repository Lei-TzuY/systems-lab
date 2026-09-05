# Systems Lab Migration Protocol & Ledger

This document is the durable preflight and migration evidence ledger for `systems-lab`.

## Status vocabulary

- **PRE-FLIGHT** — candidate selected; exact live source state still needs migration-time verification.
- **READY FOR IMPORT** — exact source head/open-PR/CI/history/hygiene gates are clean for a frozen candidate.
- **HOLD** — active implementation, CI, attribution, or external validation blocker prevents a safe import.
- **IMPORTED / VERIFIED** — non-squashed source history is retained as umbrella ancestry, selected tree matches source, and source-equivalent umbrella CI is green.
- **INTEGRATION VERIFIED** — an executable cross-project contract is permanently tested in addition to import verification.

## Candidate ledger — four-import checkpoint 2026-09-05

| Project | Layer | Frozen / observed source | Source CI evidence | Status / blocker |
| --- | --- | --- | --- | --- |
| `mini-hypervisor` | virtualization | `d32685b5453c3d1ae86ff76d0beac2b4af47094f` | CI `33972996585` + strict KVM `33972996539` | **IMPORTED / VERIFIED** |
| `minios-x86` | operating-system kernel | `e63d4218ea91069506b05944ead5a9198bf8568a` | Static `33974342423`, Tests `33974342445`, Kernel `33974342424` | **IMPORTED / VERIFIED** |
| `filesystem-lab` | filesystem/storage | `1414e9fc4646b6c482d23d0741a0e420e8fd396c` | CI `33971985113` | **IMPORTED / VERIFIED** |
| `systems-conformance-lab` | cross-cutting correctness | `b7df22b7004838b55054ec3d8d7b7a3b34df8137` | CI `33974760080` | **IMPORTED / VERIFIED** |
| `userspace-tcpip-stack` | userspace networking/protocols | observed `347820674b71f1b8203d52366604e32b0ca3fb1d` | prior source evidence successful | **HOLD** — implementation PR #330 active |
| `mini-container-runtime` | Linux container/process isolation | observed `b660e8d14aebf181e29ad844c18f7133ad0334ea` | prior source evidence successful | **HOLD** — draft implementation PR #392 active |

Every future import or refresh is rechecked immediately before execution; historical green evidence never overrides a moving head, active implementation PR, or new failure.

## Completed import 1 — `filesystem-lab`

Frozen source: `1414e9fc4646b6c482d23d0741a0e420e8fd396c`.

- bootstrap `33972535890` repeated source-head and zero-open-PR checks;
- subtree commit `8b2d286e864edbdbd22d9add82c025a9dddb9604` retains the source commit as second parent and has exact tree equality at `projects/filesystem-lab`;
- import PR #3 head `d41f34a53a18353a67225e69b2a17bc6129e4b38` passed manifest `33972592152` and verification `33972592139`;
- normal merge `16ec31643891fe6d587339f1bea543fefee2189f` then passed exact merged-main manifest `33972631312` and filesystem verification `33972631346`.

Permanent workflow is read-only. No MinIOS/filesystem interoperability is implied.

## Completed import 2 — `mini-hypervisor`

Frozen source: `d32685b5453c3d1ae86ff76d0beac2b4af47094f` after source PR #94 was normal-merged and exact source main passed both standard CI and strict real-KVM INTx proof.

- source CI `33972996585`; strict KVM `33972996539`;
- bootstrap `33973179661` repeated exact source-main and zero-open-PR checks;
- subtree commit `709291040efb288315d3d81e26b6f4e2dfe5760b` retains exact source as second parent and exact tree identity;
- final import PR #6 head `238a8695f00515ad1540fa847fbbedab880776db` passed manifest `33973305830` and verification `33973305829`;
- normal merge `f436b8863a688fcc34300577263d7dab7d00407f` passed exact merged-main manifest `33973353798` and hypervisor verification `33973353484`.

The permanent gate covers format, Clippy, tests, build, rustdoc, the source Rust 1.74 shipped-target contract, and strict real-KVM virtio-blk INTx proof. No hypervisor → MinIOS guest compatibility is claimed.

## Completed import 3 — `minios-x86`

Frozen source: `e63d4218ea91069506b05944ead5a9198bf8568a`.

Before freezing, source PR #35 was repaired rather than imported around a known correctness failure. The final design uses a dedicated per-process `waitpid_event` to wake sibling `waitpid()` callers without broadly waking unrelated waiters sharing a process channel. Exact PR-head Static, Tests, Kernel/QEMU, ASan/UBSan and mutation evidence passed before source normal merge; exact source merged-main runs then passed again.

### Pre-import evidence

- source Static `33974342423` success;
- source Tests `33974342445` success;
- source Kernel `33974342424` success, including native/QEMU regressions, host ASan/UBSan and stress-mutant kill proof;
- zero open source PRs at final freeze and migration-time bootstrap;
- configured attribution searches returned no configured disallowed matches;
- no `.gitmodules` declaration or obvious committed build/cache payload observed.

### History and umbrella evidence

- bootstrap `33975093404` rechecked source `main` and zero open PRs;
- subtree commit `56fec38f4af154e8c8dd7a993dcf70327c4ad7d0` has exact source `e63d4218...` as second parent and exact tree equality at `projects/minios-x86`;
- import PR #9 head `3a6bf88399eef68e438710c26b94658a753b9035` passed manifest `33975138166` and MinIOS verification `33975138165`;
- PR #9 normal-merged as `2ba4b6cb2b394456df5319e7b9c980ae493aace2`;
- exact merged main passed manifest `33975669912` and MinIOS verification `33975669767`, repeating build, static analysis, QEMU/native regression, ASan/UBSan, and stress-mutant proof.

The temporary write-capable bootstrap workflow and trigger were removed before the import PR. The permanent MinIOS workflow is read-only. No VM/filesystem/network interoperability is claimed by import verification.

## Completed import 4 — `systems-conformance-lab`

Frozen source: `b7df22b7004838b55054ec3d8d7b7a3b34df8137`.

The source is treated as target-independent correctness infrastructure: differential execution, fuzzing, fault injection, reduction, deterministic reproduction and target adapters. Importing the framework does not imply any target adapter already exists.

### Pre-import evidence

- exact source-main CI `33974760080` success across Ubuntu/macOS/Windows × Python 3.11/3.13;
- every matrix cell ran editable development install, `python -m pytest`, and `python -m ruff check .` successfully;
- zero open source PRs at freeze and again at bootstrap;
- configured attribution searches returned no configured disallowed matches;
- recursive tree inspection showed source/docs/tests/package metadata without `.gitmodules` or obvious committed cache/build payload.

### History and umbrella evidence

- bootstrap `33976134168` rechecked exact source main and zero open PRs, then verified tree identity;
- subtree commit `3ba8a9c3adbddb39121b82691a93573286d555e3` has exact source `b7df22b7...` as second parent and exact tree equality at `projects/systems-conformance-lab`;
- temporary write-capable bootstrap workflow and trigger were removed before the import PR;
- import PR #11 head `6ea0d5182e3432f47701197d1826b9e8b46c18d0` passed manifest `33976223333` and Conformance migration `33976223338`, including preserved ancestry/tree and all six OS/Python matrix cells;
- PR #11 normal-merged as `7f639e3ec6e12f14a370e874faad837409c0ccec`;
- exact merged main passed manifest `33976275067` and Conformance migration `33976275089`, repeating the same history/tree and six-cell source-equivalent matrix.

The permanent Conformance workflow is read-only. No conformance → target integration is claimed until a named adapter and executable regression exercise a real boundary.

## Current HOLD lanes

### `userspace-tcpip-stack`

Open implementation PR #330 (`fix(ptp-tc): fail closed on invalid timing arithmetic`) still owns source correctness work. Do not freeze or import while that implementation remains active.

### `mini-container-runtime`

Draft implementation PR #392 (`feat(image): wire OCI Entrypoint and Cmd into run admission`) explicitly remains in progress until public CLI semantics are complete and exact candidate CI is green. Do not freeze or import this source yet.

## Preflight gate

Immediately before each import or refresh:

1. record exact source `main` SHA and umbrella `main` SHA;
2. confirm no active implementation PR owns the source surface;
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

For refreshes, use a non-squashed `git subtree pull` and separately audit newly reachable source history. A current-tree copy, archive, or squash does **not** satisfy this ledger.

## Integration evidence rule

The current checkpoint verifies **four imports, zero cross-project integration edges**. That distinction is intentional.

A verified edge needs a named artifact/protocol/device/process boundary, deterministic setup where practical, executable assertions, honest platform constraints, and claims no broader than the exercised contract. Candidate examples include a MinIOS guest artifact booted by the hypervisor, a shared filesystem image contract, an explicit TAP/TUN/network device fixture, or a conformance adapter against a real target.

Phase 4 flagship status is therefore not yet claimed. The most valuable likely next edge is `mini-hypervisor` → `minios-x86`, but only executable proof can advance it.

## Original repository policy

Source repositories remain available after import. Archive/read-only decisions happen only after umbrella CI and canonical links are stable. Routine consolidation never deletes original repositories.
