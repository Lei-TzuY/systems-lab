# Systems Lab Migration Protocol & Ledger

This document is the durable preflight and migration evidence ledger for `systems-lab`.

## Status vocabulary

- **PRE-FLIGHT** — candidate selected; exact live source state still needs migration-time verification.
- **READY FOR IMPORT** — exact source head/open-PR/CI/history/hygiene gates are clean for a frozen candidate.
- **HOLD** — active implementation, CI, attribution, or external validation blocker prevents a safe import.
- **IMPORTED / VERIFIED** — non-squashed source history is retained as umbrella ancestry, selected tree matches source, and source-equivalent umbrella CI is green.
- **INTEGRATION VERIFIED** — an executable cross-project contract is permanently tested in addition to import verification.

## Candidate ledger — live preflight 2026-09-05

| Project | Layer | Exact observed `main` | Source CI evidence | Status / blocker |
| --- | --- | --- | --- | --- |
| `mini-hypervisor` | virtualization | `78ce397e587e6ef1adb0677b766ea5eeb6123a75` | CI run `33970802001` success | **HOLD** — implementation PR #94 is active |
| `minios-x86` | operating-system kernel | `0276b5326a1fbd00d2d2de26b128704b3098e42d` | Static analysis run `33534489935` success | **HOLD** — implementation PR #35 is active |
| `filesystem-lab` | filesystem/storage | `1414e9fc4646b6c482d23d0741a0e420e8fd396c` | CI run `33971985113` success | **READY FOR IMPORT** |
| `userspace-tcpip-stack` | userspace networking/protocols | `347820674b71f1b8203d52366604e32b0ca3fb1d` | Clippy run `33962740171` success | **HOLD** — implementation PR #330 is active |
| `mini-container-runtime` | Linux container/process isolation | `b660e8d14aebf181e29ad844c18f7133ad0334ea` | Tests run `33942938557` success | **HOLD** — implementation PR #392 is active |

The source CI column records the observed successful exact-main workflow relevant to this preflight. Every project is rechecked immediately before an import; a successful historical run does not override a later moving head, active implementation PR, or new failure.

### First import candidate: `filesystem-lab`

The selected freeze is `1414e9fc4646b6c482d23d0741a0e420e8fd396c`.

Pre-import evidence already established at selection time:

- no open pull request was present on the source repository;
- exact merged-main CI run `33971985113` completed successfully;
- the native source gate is `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-targets --all-features`;
- configured reachable-history commit-message searches returned no disallowed attribution matches;
- the frozen tree contains Cargo metadata, README, docs, Rust source, tests and CI configuration, with no top-level LICENSE file and no observed nested repository, vendored cache, or generated binary payload;
- the source tree state, including the absence of top-level licensing metadata, must be preserved rather than cosmetically rewritten during migration.

Selection is not import. Immediately before executing the non-squashed Git migration, recheck that source `main` still equals the selected freeze or repeat the preflight against the new head.

### Secondary systems candidate

`systems-conformance-lab` is a plausible cross-cutting conformance/integration tool and was observed at `dac29d7d97bdebe5e9b65fbd50621f7e9955582c`. It is deliberately **deferred**: Phase 0 should first prove one core source import and its permanent umbrella gate before widening scope.

Projects already owned by `compiler-runtime-lab`, including its imported debugger, libc, ELF toolchain, runtime, compiler and language-server components, are not duplicated here merely because they also touch systems topics.

## Preflight gate

Immediately before import:

1. record exact source `main` SHA and umbrella `main` SHA;
2. confirm no active implementation PR owns the same source surface;
3. confirm exact candidate CI/checks required by the source project are successful;
4. inspect recent commits so the source freeze is not accidentally taken during a moving architectural rewrite;
5. run complete reachable-history attribution scanning for the first import, or newly reachable-history scanning for a refresh;
6. preserve source README/docs/licenses and explicitly record absence of top-level licensing metadata rather than inventing it;
7. inspect generated artifacts, caches, large binaries, secrets, platform-specific fixtures, hardware/manual test requirements and nested repositories;
8. define the exact source-equivalent umbrella CI contract before merge.

## History-preserving procedure

Use non-squashed subtree operations. A source commit selected for import must remain reachable from umbrella history.

```bash
git remote add source-project https://github.com/Lei-TzuY/<project>.git
git fetch source-project --tags
git subtree add --prefix=projects/<project> source-project <frozen-sha>
```

For refreshes, use a non-squashed `git subtree pull` and separately audit newly reachable source history.

A current-tree copy, generated archive, or squash migration does **not** satisfy this ledger even if the resulting files are identical. Import status may advance to **IMPORTED / VERIFIED** only after ancestry, tree equivalence, and umbrella-path CI have all been proven.

## Integration evidence rule

Projects sharing a systems theme are not automatically integrated. A verified edge needs:

- a named artifact/protocol/device/process boundary;
- deterministic setup where practical;
- executable assertions for success and failure behavior;
- CI ownership and platform constraints documented honestly;
- no claim broader than the exercised contract.

Examples: boot image ↔ hypervisor, disk image ↔ filesystem implementation, TAP/TUN packet boundary ↔ network stack, namespace lifecycle ↔ container runtime.

## Original repository policy

Source repositories remain available after import. Archive/read-only decisions happen only after umbrella CI and canonical links are stable. Routine consolidation never deletes original repositories.
