# Binary write fault adapter

`FaultingBinaryWriter` is the first concrete fault-effect adapter layered on top of the generic `FaultSpec` / `FaultController` trigger contract. It is intentionally limited to a binary stream's logical `write` boundary so the fault is deterministic and independently testable without embedding filesystem, database, or protocol semantics into the controller.

Supported fault kinds:

- `short_write`: on the configured write occurrence, forwards at most `short_write_bytes` bytes while guaranteeing a non-empty write is shorter than the requested payload; later writes proceed normally.
- `io_error`: raises `OSError(errno.EIO)` on the configured occurrence without touching the underlying stream; later writes proceed normally.

The adapter accepts only `operation="write"` and fails closed on unsupported fault kinds or negative short-write limits. The caller retains ownership of the underlying binary stream and remains responsible for flush/fsync/close behavior and for interpreting a short return value correctly.

This is not a crash-consistency model. It does not reorder writes, corrupt already-persisted bytes, emulate kernel page cache behavior, or claim durability semantics. More specialized storage/network/process fault adapters should compose the generic trigger contract at their actual operation boundaries rather than expanding this adapter into an all-purpose fault simulator.

The integration suite runs the adapter inside a real Python child process against a real temporary file and compares its output with an unfaulted oracle through `DifferentialHarness`. That validates that an injected short write reaches the existing structured result and differential classification path as a product mismatch rather than an infrastructure failure.
