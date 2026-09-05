# Fuzz witness triage

`reduce_failure_to_repro` closes the bounded hand-off from fuzz discovery to minimized, replayable evidence without moving mutation or domain semantics into the harness.

The helper consumes a captured `FuzzFailure[bytes]`, verifies that the stored `ComparisonResult` still agrees with the stored stable `FailureSignature`, and then delegates deterministic shrinking to `reduce_case`. The live `DifferentialHarness` is used as the preservation predicate, so a candidate is accepted only when it reproduces the exact original stable signature. Product mismatches and infrastructure failures therefore cannot silently cross classes during reduction.

After reduction, `DifferentialHarness.write_repro` re-executes the minimized input and performs one final expected-signature check before publishing the bundle. Harness replay-context binding, input hashing, bounded loading, archive interoperability, and retention rules continue to apply unchanged.

```python
result = reduce_failure_to_repro(
    discovery.failures[0],
    harness=harness,
    destination=repro_dir,
    candidates=hierarchical_byte_deletions,
    max_evaluations=100,
)
```

The helper deliberately does not choose a mutation engine, semantic reducer, retention policy, or storage service. Callers supply deterministic reduction candidates and may replace `len` with another non-negative measure when their byte representation needs a different size contract.
