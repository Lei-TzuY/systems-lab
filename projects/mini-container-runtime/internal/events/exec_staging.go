package events

import (
	"fmt"
	"time"
)

const (
	EventExecExit   EventType = "exec_exit"
	EventExecFailed EventType = "exec_failed"
)

type stagedExecEvent struct {
	containerID  string
	image        string
	message      string
	containerPID int
	pidStartTime uint64
	command      []string
}

var stagedExecs = make(map[string]stagedExecEvent)
var activeExecs = make(map[string]stagedExecEvent)

func HasPendingExec() bool {
	mu.Lock()
	defer mu.Unlock()
	return len(stagedExecs) != 0
}

func stageExecEvent(containerID, image, message string) error {
	if containerID == "" {
		return fmt.Errorf("exec event container ID is empty")
	}
	if _, ok := stagedExecs[containerID]; ok {
		return fmt.Errorf("exec event for container %s is already staged", containerID)
	}
	if _, ok := activeExecs[containerID]; ok {
		return fmt.Errorf("exec event for container %s is already active", containerID)
	}
	stagedExecs[containerID] = stagedExecEvent{containerID: containerID, image: image, message: message}
	return nil
}

// BindPendingExecAttribution attaches the exact container process generation and
// argv to the single staged exec intent. It is called only after the runtime has
// verified the persisted PID start time, so terminal events cannot be confused
// with a later restart that reuses the same container ID or numeric PID.
func BindPendingExecAttribution(containerPID int, pidStartTime uint64, command []string) error {
	mu.Lock()
	defer mu.Unlock()
	if len(stagedExecs) == 0 {
		return nil
	}
	if len(stagedExecs) != 1 {
		return fmt.Errorf("cannot bind exec attribution: %d staged exec events", len(stagedExecs))
	}
	if containerPID <= 0 || pidStartTime == 0 {
		return fmt.Errorf("invalid exec process generation pid=%d start_time=%d", containerPID, pidStartTime)
	}
	for containerID, staged := range stagedExecs {
		staged.containerPID = containerPID
		staged.pidStartTime = pidStartTime
		staged.command = append([]string(nil), command...)
		stagedExecs[containerID] = staged
	}
	return nil
}

func execLifecycleEvent(staged stagedExecEvent, typ EventType, message string) Event {
	return Event{
		Timestamp:             time.Now(),
		Type:                  typ,
		ContainerID:           staged.containerID,
		Image:                 staged.image,
		Message:               message,
		ContainerPID:          staged.containerPID,
		ContainerPIDStartTime: staged.pidStartTime,
		Command:               append([]string(nil), staged.command...),
	}
}

func CommitPendingExec() error {
	mu.Lock()
	defer mu.Unlock()
	if len(stagedExecs) == 0 {
		return nil
	}
	if len(stagedExecs) != 1 {
		return fmt.Errorf("cannot commit exec event: %d staged exec events", len(stagedExecs))
	}
	if len(activeExecs) != 0 {
		return fmt.Errorf("cannot commit exec event: %d active exec events", len(activeExecs))
	}

	var staged stagedExecEvent
	for _, candidate := range stagedExecs {
		staged = candidate
	}

	if err := appendEventUnlocked(execLifecycleEvent(staged, EventExec, staged.message)); err != nil {
		return err
	}
	delete(stagedExecs, staged.containerID)
	activeExecs[staged.containerID] = staged
	return nil
}

func CompletePendingExec(exitCode int, detail string) error {
	mu.Lock()
	defer mu.Unlock()
	if len(activeExecs) == 0 {
		return nil
	}
	if len(activeExecs) != 1 {
		return fmt.Errorf("cannot complete exec event: %d active exec events", len(activeExecs))
	}
	var active stagedExecEvent
	for _, candidate := range activeExecs {
		active = candidate
	}
	delete(activeExecs, active.containerID)
	message := fmt.Sprintf("%s; exit_code=%d", active.message, exitCode)
	if detail != "" {
		message += "; " + detail
	}
	code := exitCode
	evt := execLifecycleEvent(active, EventExecExit, message)
	evt.ExitCode = &code
	evt.Error = detail
	return appendEventUnlocked(evt)
}

func FailPendingExec(detail string) error {
	mu.Lock()
	defer mu.Unlock()
	if len(stagedExecs) == 0 {
		return nil
	}
	if len(stagedExecs) != 1 {
		return fmt.Errorf("cannot fail exec event: %d staged exec events", len(stagedExecs))
	}
	var staged stagedExecEvent
	for _, candidate := range stagedExecs {
		staged = candidate
	}
	delete(stagedExecs, staged.containerID)
	message := staged.message
	if detail != "" {
		message += "; " + detail
	}
	evt := execLifecycleEvent(staged, EventExecFailed, message)
	evt.Error = detail
	return appendEventUnlocked(evt)
}

func DiscardPendingExec() error {
	mu.Lock()
	defer mu.Unlock()
	if len(stagedExecs) == 0 {
		return nil
	}
	if len(stagedExecs) != 1 {
		return fmt.Errorf("cannot discard exec event: %d staged exec events", len(stagedExecs))
	}
	for containerID := range stagedExecs {
		delete(stagedExecs, containerID)
	}
	return nil
}
