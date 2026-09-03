# Multi-Agent Collaboration Guidelines & Engineering Standards

This document establishes the collaboration contract and engineering standards for all AI coding agents (Antigravity, Codex, Claude, etc.) operating concurrently on the `userspace-tcpip-stack` repository.

---

## 1. Multi-Agent Git Synchronization Protocol

To ensure seamless concurrent development without overwriting or losing peer agents' work:

1. **Fetch Before Starting**:
   - Always inspect remote updates before beginning a task:
     ```powershell
     git fetch origin --prune
     ```
   - Check if `origin/main` has advanced and merge/rebase cleanly.

2. **Strictly Prohibit Force-Push**:
   - **NEVER** run `git push --force` or `git push -f`.
   - All integrations must be forward-only or clean merge commits.

3. **Additive Merge Conflict Resolution**:
   - When resolving conflicts (e.g. in `src/lib.rs` or shared modules), **NEVER discard or overwrite another agent's implementations or tests**.
   - Carefully synthesize both sets of changes: preserve peer agents' fixes, boundary checks, and test cases, while integrating your own.

4. **Maintain Clean Working Tree**:
   - Never leave untracked scratch scripts or half-committed changes in the repository.
   - Verify `git status` shows `nothing to commit, working tree clean` before ending turns.

---

## 2. Core Repository Architectural Constraints

1. **Pure Rust with Zero External Dependencies**:
   - The `[dependencies]` table in `Cargo.toml` must **remain completely empty**.
   - All data structures, parsers, serializers, timers, cryptographic/checksum routines, and servos must be implemented in pure standard Rust (`std` / `core`).

2. **Modular File Architecture**:
   - Implement new protocols and subsystems in dedicated, modular files under `src/` (e.g. `src/tsn_*.rs`, `src/ptp_*.rs`, `src/oran_*.rs`).
   - Register new modules and re-export public APIs cleanly in [`src/lib.rs`](src/lib.rs).

3. **Targeted Integration Testing**:
   - Every feature must be accompanied by comprehensive tests under `tests/test_<feature>.rs`.
   - Run targeted tests using `cargo test --test <name>` to optimize compilation and test execution on Windows.
   - Ensure all affected test suites pass with 100% success.

4. **Conventional Commits**:
   - Use clear conventional commit messages:
     - `feat(<subsystem>): <description>`
     - `fix(<subsystem>): <description>`
     - `test(<subsystem>): <description>`
