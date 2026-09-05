# Deterministic byte reduction candidates

`hierarchical_byte_deletions()` is the first reusable concrete candidate strategy layered on the generic `reduce_case()` engine. It is intentionally byte-oriented and semantics-free: adapters with structured formats should continue to supply domain-aware candidates instead of teaching the core reducer a protocol.

The generator walks contiguous deletion widths from coarse to fine, ending at single-byte deletion. Candidate order is deterministic, every emitted candidate is strictly smaller than its input, and duplicates are suppressed in first-seen order. These properties match `reduce_case()`'s first-improvement contract and let a failing opaque byte stream shed large irrelevant regions before paying for fine-grained checks.

The real-process pipeline integration uses this strategy against a candidate that mishandles the byte token `BUG` and an oracle that echoes input. The failure is reduced from `prefix BUG suffix` to `BUG` while preserving the exact stable failure signature, then persisted as a validated repro bundle. This keeps the capability tied to the runner → comparator → failure identity → reducer → repro path rather than validating a synthetic reducer in isolation.
