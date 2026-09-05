import pytest

from systems_conformance.fault import FaultController, FaultSpec


def test_fault_triggers_at_configured_matching_occurrence() -> None:
    spec = FaultSpec(operation="write", occurrence=2, kind="enospc")
    controller = FaultController(spec)

    assert controller.checkpoint("write") is None
    assert controller.checkpoint("read") is None
    assert controller.checkpoint("write") is None
    assert controller.checkpoint("write") is spec
    assert controller.triggered is True
    assert controller.matching_occurrences == 3


def test_fault_is_single_shot_after_trigger() -> None:
    spec = FaultSpec(operation="commit", occurrence=0, kind="crash")
    controller = FaultController(spec)

    assert controller.checkpoint("commit") is spec
    assert controller.checkpoint("commit") is None
    assert controller.matching_occurrences == 1


def test_unrelated_operations_do_not_advance_occurrence() -> None:
    spec = FaultSpec(operation="send", occurrence=1, kind="drop")
    controller = FaultController(spec)

    assert controller.checkpoint("recv") is None
    assert controller.checkpoint("recv") is None
    assert controller.matching_occurrences == 0
    assert controller.checkpoint("send") is None
    assert controller.checkpoint("send") is spec


def test_fault_spec_is_reusable_for_reproducible_runs() -> None:
    spec = FaultSpec(operation="alloc", occurrence=1, kind="oom")

    first = FaultController(spec)
    second = FaultController(spec)

    first_trace = [first.checkpoint("alloc"), first.checkpoint("alloc")]
    second_trace = [second.checkpoint("alloc"), second.checkpoint("alloc")]

    assert first_trace == second_trace == [None, spec]


@pytest.mark.parametrize(
    ("kwargs", "message"),
    [
        ({"operation": "", "occurrence": 0, "kind": "fail"}, "operation must be non-empty"),
        (
            {"operation": "write", "occurrence": -1, "kind": "fail"},
            "occurrence must be non-negative",
        ),
        ({"operation": "write", "occurrence": 0, "kind": ""}, "kind must be non-empty"),
    ],
)
def test_fault_spec_rejects_invalid_configuration(
    kwargs: dict[str, object], message: str
) -> None:
    with pytest.raises(ValueError, match=message):
        FaultSpec(**kwargs)  # type: ignore[arg-type]


def test_checkpoint_rejects_empty_operation() -> None:
    controller = FaultController(FaultSpec(operation="write", occurrence=0, kind="fail"))

    with pytest.raises(ValueError, match="operation must be non-empty"):
        controller.checkpoint("")
