# SQLite query target adapter

`SQLiteQueryTarget` is the first concrete conformance-domain adapter built above the generic process/differential substrate. It runs the Python standard-library `sqlite3` engine in a fresh child process and a fresh in-memory database for every evaluation, then returns canonical JSON rows through the existing `CommandTarget` boundary.

The input is one strict UTF-8 JSON object with `setup`, `query`, and optional `params` fields:

```json
{"setup":["CREATE TABLE t(v INTEGER)","INSERT INTO t VALUES (1)"],"query":"SELECT v FROM t","params":[]}
```

`setup` is an ordered list of single SQL statements. `query` is one non-empty SQL statement. `params` is a positional JSON-scalar array. Successful output is compact JSON containing `columns` and `rows`; SQLite BLOB values are normalized as `{"$blob":"<lowercase hex>"}` so byte values remain deterministic and JSON-safe.

The request decoder rejects duplicate object fields instead of accepting last-key-wins ambiguity. It also rejects the non-standard JSON constants `NaN`, `Infinity`, and `-Infinity`, floating-point values that overflow to a non-finite Python value, and integer parameters outside SQLite's signed 64-bit binding range. These cases are reported as deterministic `protocol_error` results rather than leaking Python parser/binding quirks or uncaught tracebacks into differential signatures.

Each evaluation is process-isolated and inherits the shared runner's timeout, process-tree cleanup, capture limits, and hard aggregate output budget. The worker explicitly disables extension loading and denies `ATTACH`, `DETACH`, and input-issued `PRAGMA` operations through SQLite's authorizer so untrusted test cases cannot intentionally bind arbitrary host database paths or mutate adapter configuration. SQL can still be computationally expensive, which is why callers must retain finite harness timeouts.

`foreign_keys` is part of the adapter configuration and therefore part of the resulting `CommandTarget` argv/replay identity:

```python
from systems_conformance import DifferentialHarness, SQLiteQueryTarget

candidate = SQLiteQueryTarget(foreign_keys=True).as_command_target()
oracle = SQLiteQueryTarget(foreign_keys=False).as_command_target()
harness = DifferentialHarness(candidate=candidate, oracle=oracle, timeout_seconds=2.0)
```

The adapter deliberately does not define SQL equivalence, generate SQL, rewrite unordered results, persist databases between cases, or hide SQLite errors. Those semantics belong in future bounded domain slices when a concrete conformance campaign requires them. This adapter's contract is narrower: deterministic protocol decoding, safe process isolation, canonical observable rows, and composition with the existing differential/fuzz/repro pipeline.
