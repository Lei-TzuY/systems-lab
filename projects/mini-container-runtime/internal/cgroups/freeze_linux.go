//go:build linux

// internal/cgroups/freeze_linux.go
//
// Container Pause & Unpause (Freezer Controller)
// ───────────────────────────────────────────────
// cgroups v2 provides a unified process freezer via `/sys/fs/cgroup/<name>/cgroup.freeze`.
// Writing cgroup.freeze requests a transition; cgroup.events reports when the
// kernel has actually completed it. Callers must not treat the write alone as
// proof that every task is frozen or resumed.

package cgroups

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"
)

const (
	freezeAckTimeout      = 5 * time.Second
	freezeAckPollInterval = 10 * time.Millisecond
)

// Freeze pauses all processes in the given cgroup and returns only after the
// kernel reports `frozen 1` in cgroup.events.
func Freeze(name string) error {
	return setFreeze(name, "1")
}

// Unfreeze resumes all processes in the given cgroup and returns only after the
// kernel reports `frozen 0` in cgroup.events.
func Unfreeze(name string) error {
	return setFreeze(name, "0")
}

// IsFrozen checks whether the kernel currently reports the cgroup as frozen.
// cgroup.freeze reflects the requested state and may become 1 before all tasks
// have actually stopped, so completion is read from cgroup.events instead.
func IsFrozen(name string) (bool, error) {
	if err := validateCgroupName(name); err != nil {
		return false, err
	}
	cgPath := filepath.Join(cgroupV2Root, name)
	data, err := os.ReadFile(filepath.Join(cgPath, "cgroup.events"))
	if err != nil {
		return false, fmt.Errorf("read cgroup.events: %w", err)
	}
	return parseFrozenEvent(data)
}

func setFreeze(name, value string) error {
	if err := validateCgroupName(name); err != nil {
		return err
	}
	wantFrozen, err := freezeValue(value)
	if err != nil {
		return err
	}
	cgPath := filepath.Join(cgroupV2Root, name)
	attempts := int(freezeAckTimeout/freezeAckPollInterval) + 1
	return setFreezeAt(cgPath, value, wantFrozen, attempts, freezeAckPollInterval, os.WriteFile, os.ReadFile, time.Sleep)
}

func setFreezeAt(
	cgPath, value string,
	wantFrozen bool,
	attempts int,
	pollInterval time.Duration,
	writeFile func(string, []byte, os.FileMode) error,
	readFile func(string) ([]byte, error),
	sleep func(time.Duration),
) error {
	if writeFile == nil || readFile == nil || sleep == nil {
		return fmt.Errorf("cgroup freezer I/O dependency is nil")
	}
	if attempts <= 0 {
		return fmt.Errorf("cgroup freezer acknowledgement attempts must be positive")
	}
	freezePath := filepath.Join(cgPath, "cgroup.freeze")
	if err := writeFile(freezePath, []byte(value), 0644); err != nil {
		return fmt.Errorf("write %s to %s: %w", value, freezePath, err)
	}

	eventsPath := filepath.Join(cgPath, "cgroup.events")
	for attempt := 0; attempt < attempts; attempt++ {
		data, err := readFile(eventsPath)
		if err != nil {
			return fmt.Errorf("read cgroup freezer acknowledgement: %w", err)
		}
		frozen, err := parseFrozenEvent(data)
		if err != nil {
			return err
		}
		if frozen == wantFrozen {
			return nil
		}
		if attempt+1 < attempts {
			sleep(pollInterval)
		}
	}
	return fmt.Errorf("timed out waiting for cgroup frozen=%t acknowledgement", wantFrozen)
}

func freezeValue(value string) (bool, error) {
	switch value {
	case "0":
		return false, nil
	case "1":
		return true, nil
	default:
		return false, fmt.Errorf("invalid cgroup.freeze value %q", value)
	}
}

func parseFrozenEvent(data []byte) (bool, error) {
	found := false
	frozen := false
	for _, line := range strings.Split(string(data), "\n") {
		fields := strings.Fields(line)
		if len(fields) == 0 || fields[0] != "frozen" {
			continue
		}
		if found {
			return false, fmt.Errorf("duplicate frozen entry in cgroup.events")
		}
		if len(fields) != 2 {
			return false, fmt.Errorf("malformed frozen entry in cgroup.events: %q", line)
		}
		switch fields[1] {
		case "0":
			frozen = false
		case "1":
			frozen = true
		default:
			return false, fmt.Errorf("unexpected cgroup.events frozen value %q", fields[1])
		}
		found = true
	}
	if !found {
		return false, fmt.Errorf("cgroup.events is missing frozen state")
	}
	return frozen, nil
}
