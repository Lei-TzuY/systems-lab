import sys

from systems_conformance import CommandTarget, DifferentialHarness, run_feedback_guided_campaign

TRACED_ECHO = r'''
import sys

seen = set()
def trace(frame, event, arg):
    if event == "line" and frame.f_code.co_name == "exercise":
        seen.add(frame.f_lineno)
    return trace

def exercise(data):
    marker = 0
    if data.startswith(b"A"):
        marker += 1
    if data.endswith(b"Z"):
        marker += 2
    return data, marker

sys.settrace(trace)
data, _ = exercise(sys.stdin.buffer.read())
sys.settrace(None)
sys.stdout.buffer.write(data)
sys.stderr.write("COV:" + ",".join(str(line) for line in sorted(seen)))
'''

TRACED_CANDIDATE = TRACED_ECHO.replace(
    "sys.stdout.buffer.write(data)",
    "sys.stdout.buffer.write(b'bad' if data == b'AZ' else data)",
)


def test_feedback_campaign_grows_from_real_trace_features_and_captures_failure() -> None:
    candidate = CommandTarget((sys.executable, "-c", TRACED_CANDIDATE))
    oracle = CommandTarget((sys.executable, "-c", TRACED_ECHO))
    harness = DifferentialHarness(candidate=candidate, oracle=oracle)

    def evaluate(case: bytes):
        run = harness.evaluate(case)
        prefix, payload = run.candidate.stderr.text.split(":", 1)
        assert prefix == "COV"
        features = {int(value) for value in payload.split(",") if value}
        return run.comparison, features

    def mutate(case: bytes, index: int) -> bytes:
        return b"A" + case if index == 0 else case + b"Z"

    result = run_feedback_guided_campaign(
        seeds=(b"",),
        mutate=mutate,
        evaluate=evaluate,
        mutations_per_case=2,
        max_evaluations=7,
    )

    assert [entry.case for entry in result.corpus] == [b"", b"A", b"Z"]
    assert len(result.features) >= 6
    assert result.evaluations == 7
    assert result.exhausted_budget
    assert len(result.failures) == 1
    assert result.failures[0].case == b"AZ"
    assert result.failures[0].evaluation_index == 4
    assert result.failures[0].comparison.classification == "product_mismatch"
    assert result.failures[0].signature.kind == "product_mismatch"
