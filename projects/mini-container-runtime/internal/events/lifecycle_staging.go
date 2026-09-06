package events

import (
	"fmt"
	"time"
)

type stagedStartEvent struct {
	containerID string
	image       string
	message     string
}

// stagedStarts and committedStarts are process-local lifecycle proofs guarded by
// mu in events.go. A runtime process can execute multiple restart generations
// serially for one container ID; every generation must therefore complete its
// own start/die pair before the next start can be committed.
var (
	stagedStarts    = make(map[string]stagedStartEvent)
	committedStarts = make(map[string]struct{})
)

func stageStartEvent(containerID, image, message string) error {
	if containerID == "" {
		return fmt.Errorf("start event container ID is empty")
	}
	if _, ok := committedStarts[containerID]; ok {
		return fmt.Errorf("start event for container %s is already committed", containerID)
	}
	if _, ok := stagedStarts[containerID]; ok {
		return fmt.Errorf("start event for container %s is already staged", containerID)
	}
	stagedStarts[containerID] = stagedStartEvent{
		containerID: containerID,
		image:       image,
		message:     message,
	}
	return nil
}

// StageRuntimeStart gives the authoritative runtime attempt its own pending
// start proof. A matching CLI pre-stage is accepted as an idempotent handoff so
// legacy callers cannot create duplicate events. A committed start means the
// previous generation has not yet closed with Die and is rejected fail-closed.
func StageRuntimeStart(containerID, image, message string) error {
	mu.Lock()
	defer mu.Unlock()

	if containerID == "" {
		return nil
	}
	if _, ok := committedStarts[containerID]; ok {
		return fmt.Errorf("cannot stage next runtime start for container %s before previous die", containerID)
	}
	if existing, ok := stagedStarts[containerID]; ok {
		if existing.image == image && existing.message == message {
			return nil
		}
		return fmt.Errorf("conflicting staged start for container %s", containerID)
	}
	stagedStarts[containerID] = stagedStartEvent{
		containerID: containerID,
		image:       image,
		message:     message,
	}
	return nil
}

// CancelPendingStart removes an uncommitted attempt proof after pre-exec setup
// fails. A committed generation is deliberately untouched; only Die may close
// that lifecycle proof.
func CancelPendingStart(containerID string) {
	mu.Lock()
	defer mu.Unlock()
	delete(stagedStarts, containerID)
}

// CommitPendingStart publishes the single staged start event after the runtime
// parent has observed READY followed by CLOEXEC EOF from the init process. No
// pending event is a no-op. Multiple pending events are ambiguous and fail
// closed so one exec proof can never be attributed to another container.
func CommitPendingStart() error {
	mu.Lock()
	defer mu.Unlock()

	if len(stagedStarts) == 0 {
		return nil
	}
	if len(stagedStarts) != 1 {
		return fmt.Errorf("cannot commit runtime start: %d staged start events", len(stagedStarts))
	}

	var staged stagedStartEvent
	for _, candidate := range stagedStarts {
		staged = candidate
	}
	if err := appendEventUnlocked(Event{
		Timestamp:   time.Now(),
		Type:        EventStart,
		ContainerID: staged.containerID,
		Image:       staged.image,
		Message:     staged.message,
	}); err != nil {
		return err
	}
	delete(stagedStarts, staged.containerID)
	committedStarts[staged.containerID] = struct{}{}
	return nil
}

func consumeDieProof(containerID string) bool {
	if _, ok := committedStarts[containerID]; !ok {
		delete(stagedStarts, containerID)
		return false
	}
	return true
}

func finishDieProof(containerID string) {
	delete(committedStarts, containerID)
}
