# Systems Lab Migration Protocol & Ledger

This document is the durable preflight and migration evidence ledger for `systems-lab`.

## Status vocabulary

- **PRE-FLIGHT** — candidate selected; exact live source state still needs migration-time verification.
- **READY FOR IMPORT** — exact source head/open-PR/CI/history/hygiene gates are clean for a frozen candidate.
- **HOLD** — active implementation, CI, attribution, or external validation blocker prevents a safe import.
- **IMPORTED / VERIFIED** — non-squashed source history is retained as umbrella ancestry, selected tree matches source, and source-equivalent umbrella CI is green.
- **INTEGRATION VERIFIED** — an executable cross-project contract is permanently tested in addition to import verification.

## Candidate ledger — 2026-09-05

| Project | Layer | Exact live freeze | Status |
| --- | --- | --- | --- |
| mini-hypervisor | virtualization | pending live preflight | PRE-FLIGHT |
| minios-x86 | operating-system kernel | pending live preflight | PRE-FLIGHT |
| filesystem-lab | filesystem/storage | pending live preflight | PRE-FLIGHT |
| userspace-tcpip-stack | userspace networking/protocols | pending live preflight | PRE-FLIGHT |
| mini-container-runtime | Linux container/process isolation | pending live preflight | PRE-FLIGHT |

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
