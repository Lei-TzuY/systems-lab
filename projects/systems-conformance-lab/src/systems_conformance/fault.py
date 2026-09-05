from dataclasses import dataclass


@dataclass(frozen=True, slots=True)
class FaultSpec:
    """Deterministic fault trigger description for target-specific adapters."""

    operation: str
    occurrence: int
    kind: str

    def __post_init__(self) -> None:
        if not self.operation:
            raise ValueError("operation must be non-empty")
        if self.occurrence < 0:
            raise ValueError("occurrence must be non-negative")
        if not self.kind:
            raise ValueError("kind must be non-empty")


class FaultController:
    """Single-shot deterministic fault trigger controller.

    Adapters call ``checkpoint`` at explicit logical operations. Only matching
    operations advance the occurrence counter. Once the configured occurrence
    is reached, the controller returns the immutable ``FaultSpec`` exactly once.

    The controller deliberately does not perform side effects itself. Adapters
    decide how a fault kind maps to an injected exception, short write, dropped
    message, allocation failure, or other system-specific behavior.
    """

    __slots__ = ("_matching_occurrences", "_spec", "_triggered")

    def __init__(self, spec: FaultSpec) -> None:
        self._spec = spec
        self._matching_occurrences = 0
        self._triggered = False

    @property
    def triggered(self) -> bool:
        return self._triggered

    @property
    def matching_occurrences(self) -> int:
        return self._matching_occurrences

    def checkpoint(self, operation: str) -> FaultSpec | None:
        """Return the configured fault exactly at its matching occurrence."""

        if not operation:
            raise ValueError("operation must be non-empty")
        if self._triggered or operation != self._spec.operation:
            return None

        occurrence = self._matching_occurrences
        self._matching_occurrences += 1
        if occurrence != self._spec.occurrence:
            return None

        self._triggered = True
        return self._spec
