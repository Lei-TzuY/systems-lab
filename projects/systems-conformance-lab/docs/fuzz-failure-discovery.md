# Deterministic fuzz failure discovery

`run_failure_discovery_campaign` extends the bounded deterministic fuzz substrate from first-failure search to distinct failure-class discovery. It receives the same index-driven case source and `ComparisonResult` evaluator as `run_fuzz_campaign`, but continues after failures and records only the first witness for each stable `FailureSignature`.

The campaign is bounded by both `max_evaluations` and `max_unique_failures`. Ordering is deterministic: findings appear in the order their signatures are first observed, and duplicate signatures do not consume the unique-failure budget. Product mismatches and infrastructure failures remain separate because their stable signature kind and dimensions are preserved unchanged.

This layer intentionally does not rank failures, mutate a corpus, perform coverage guidance, reduce inputs, or persist reproducers. A caller can feed each returned first witness into the existing reducer and repro pipeline independently. Exceptions from case generation/evaluation and inconsistent comparison records remain harness failures and are not converted into fuzz findings.

The integration suite validates the contract through real Python candidate/oracle child processes: two inputs that produce the same stdout mismatch collapse to one finding, while a later exit-code-plus-stdout mismatch becomes a second distinct finding within the same deterministic evaluation budget.
