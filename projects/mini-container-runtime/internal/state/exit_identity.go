package state

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"
)

const (
	exitedIdentitySuffix         = ".exit"
	exitedIdentityRequiredSuffix = ".exit-required"
)

type exitedIdentity struct {
	PID          int    `json:"pid"`
	PIDStartTime uint64 `json:"pid_start_time"`
}

func validateExitedIdentity(identity exitedIdentity) error {
	if identity.PID <= 0 || identity.PIDStartTime == 0 {
		return fmt.Errorf("invalid persisted exited process identity %d/%d", identity.PID, identity.PIDStartTime)
	}
	return nil
}

func exitedIdentityPath(containerDir, id string) string {
	return filepath.Join(containerDir, id+exitedIdentitySuffix)
}

func exitedIdentityRequiredPath(containerDir, id string) string {
	return filepath.Join(containerDir, id+exitedIdentityRequiredSuffix)
}

// writeExitedIdentityUnlocked is retained for upgrade-compatibility tests and
// legacy records. Modern stopped transitions embed the exact identity directly
// in the atomic container JSON and never publish a new .exit sidecar.
func (s *Store) writeExitedIdentityUnlocked(id string, pid int, pidStartTime uint64) error {
	if err := validateID(id); err != nil {
		return err
	}
	identity := exitedIdentity{PID: pid, PIDStartTime: pidStartTime}
	if err := validateExitedIdentity(identity); err != nil {
		return err
	}
	data, err := json.Marshal(identity)
	if err != nil {
		return fmt.Errorf("marshal exited process identity: %w", err)
	}
	if err := atomicWriteFile(s.ctrDir, exitedIdentityPath(s.ctrDir, id), data); err != nil {
		return fmt.Errorf("persist exited process identity: %w", err)
	}
	return nil
}

func (s *Store) readExitedIdentityUnlocked(id string) (exitedIdentity, bool, error) {
	if err := validateID(id); err != nil {
		return exitedIdentity{}, false, err
	}
	path := exitedIdentityPath(s.ctrDir, id)
	data, err := readRegularStateFile(path, "exited process identity")
	if err != nil {
		if os.IsNotExist(err) {
			return exitedIdentity{}, false, nil
		}
		return exitedIdentity{}, false, fmt.Errorf("read exited process identity: %w", err)
	}
	var identity exitedIdentity
	if err := json.Unmarshal(data, &identity); err != nil {
		return exitedIdentity{}, false, fmt.Errorf("unmarshal exited process identity: %w", err)
	}
	if err := validateExitedIdentity(identity); err != nil {
		return exitedIdentity{}, false, err
	}
	return identity, true, nil
}

func (s *Store) containerEmbeddedExitedIdentityUnlocked(id string) (exitedIdentity, bool, error) {
	snapshot, err := s.readStoppedGenerationTeardownSnapshotUnlocked(id)
	if err != nil {
		return exitedIdentity{}, false, err
	}
	return snapshot.identity, snapshot.identityEmbedded, nil
}

func (s *Store) containerExitIdentityRequirementUnlocked(id string) (required bool, present bool, err error) {
	snapshot, err := s.readStoppedGenerationTeardownSnapshotUnlocked(id)
	if err != nil {
		return false, false, err
	}
	return snapshot.required, snapshot.requirementPresent, nil
}

func (s *Store) exitedIdentityRequiredUnlocked(id string) (bool, error) {
	if err := validateID(id); err != nil {
		return false, err
	}
	path := exitedIdentityRequiredPath(s.ctrDir, id)
	data, err := readRegularStateFile(path, "exited identity requirement")
	if err != nil {
		if os.IsNotExist(err) {
			return false, nil
		}
		return false, fmt.Errorf("read exited identity requirement: %w", err)
	}
	if string(data) != "1\n" {
		return false, fmt.Errorf("invalid persisted exited identity requirement")
	}
	return true, nil
}

func (s *Store) readCurrentExitedIdentityUnlocked(id string) (exitedIdentity, bool, error) {
	snapshot, err := s.readStoppedGenerationTeardownSnapshotUnlocked(id)
	if err != nil {
		return exitedIdentity{}, false, err
	}
	if snapshot.identityEmbedded {
		if !snapshot.requirementPresent || !snapshot.required {
			return exitedIdentity{}, false, fmt.Errorf("persisted exit identity exists without required lifecycle capability")
		}
		return snapshot.identity, true, nil
	}
	if snapshot.versioned {
		return exitedIdentity{}, false, fmt.Errorf("versioned stopped generation is missing embedded exit identity")
	}
	return s.readExitedIdentityUnlocked(id)
}

func (s *Store) GetExitedIdentity(id string) (pid int, pidStartTime uint64, ok bool, err error) {
	if s == nil {
		return 0, 0, false, fmt.Errorf("state store is nil")
	}
	if err := validateID(id); err != nil {
		return 0, 0, false, err
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	if err := lockStateFile(s.lockFile); err != nil {
		return 0, 0, false, err
	}
	defer func() { _ = unlockStateFile(s.lockFile) }()

	identity, ok, err := s.readCurrentExitedIdentityUnlocked(id)
	if err != nil || !ok {
		return 0, 0, ok, err
	}
	return identity.PID, identity.PIDStartTime, true, nil
}

func (s *Store) GetExitedIdentityForStoppedRevision(id string, revision uint64) (pid int, pidStartTime uint64, current bool, ok bool, err error) {
	if s == nil {
		return 0, 0, false, false, fmt.Errorf("state store is nil")
	}
	if err := validateID(id); err != nil {
		return 0, 0, false, false, err
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	if err := lockStateFile(s.lockFile); err != nil {
		return 0, 0, false, false, err
	}
	defer func() { _ = unlockStateFile(s.lockFile) }()

	snapshot, current, err := s.readStoppedGenerationTeardownSnapshotForRevisionUnlocked(id, revision)
	if err != nil || !current {
		return 0, 0, current, false, err
	}
	if snapshot.identityEmbedded {
		if !snapshot.requirementPresent || !snapshot.required {
			return 0, 0, true, false, fmt.Errorf("persisted exit identity exists without required lifecycle capability")
		}
		return snapshot.identity.PID, snapshot.identity.PIDStartTime, true, true, nil
	}
	if snapshot.versioned {
		return 0, 0, true, false, fmt.Errorf("versioned stopped generation is missing embedded exit identity")
	}
	identity, ok, err := s.readExitedIdentityUnlocked(id)
	if err != nil || !ok {
		return 0, 0, true, ok, err
	}
	return identity.PID, identity.PIDStartTime, true, true, nil
}

func (s *Store) GetStoppedExitIdentityPolicy(id string, revision uint64) (pid int, pidStartTime uint64, current bool, ok bool, required bool, err error) {
	if s == nil {
		return 0, 0, false, false, false, fmt.Errorf("state store is nil")
	}
	if err := validateID(id); err != nil {
		return 0, 0, false, false, false, err
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	if err := lockStateFile(s.lockFile); err != nil {
		return 0, 0, false, false, false, err
	}
	defer func() { _ = unlockStateFile(s.lockFile) }()

	snapshot, current, err := s.readStoppedGenerationTeardownSnapshotForRevisionUnlocked(id, revision)
	if err != nil || !current {
		return 0, 0, current, false, false, err
	}
	if snapshot.versioned {
		if !snapshot.requirementPresent || !snapshot.required {
			return 0, 0, true, false, false, fmt.Errorf("versioned stopped generation is missing required exit identity capability")
		}
		if !snapshot.identityEmbedded {
			return 0, 0, true, false, true, fmt.Errorf("versioned stopped generation is missing embedded exit identity")
		}
		return snapshot.identity.PID, snapshot.identity.PIDStartTime, true, true, true, nil
	}
	if snapshot.identityEmbedded {
		if !snapshot.requirementPresent || !snapshot.required {
			return 0, 0, true, false, false, fmt.Errorf("persisted exit identity exists without required lifecycle capability")
		}
		return snapshot.identity.PID, snapshot.identity.PIDStartTime, true, true, true, nil
	}

	identity, ok, err := s.readExitedIdentityUnlocked(id)
	if err != nil {
		return 0, 0, true, false, false, err
	}
	if ok {
		return identity.PID, identity.PIDStartTime, true, true, true, nil
	}
	if snapshot.requirementPresent {
		return 0, 0, true, false, snapshot.required, nil
	}

	required, err = s.exitedIdentityRequiredUnlocked(id)
	if err != nil {
		return 0, 0, true, false, false, err
	}
	return 0, 0, true, false, required, nil
}

func (s *Store) StoppedRevisionRequiresExitedIdentity(id string, revision uint64) (current bool, required bool, err error) {
	if s == nil {
		return false, false, fmt.Errorf("state store is nil")
	}
	if err := validateID(id); err != nil {
		return false, false, err
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	if err := lockStateFile(s.lockFile); err != nil {
		return false, false, err
	}
	defer func() { _ = unlockStateFile(s.lockFile) }()

	snapshot, current, err := s.readStoppedGenerationTeardownSnapshotForRevisionUnlocked(id, revision)
	if err != nil || !current {
		return current, false, err
	}
	if snapshot.versioned {
		if !snapshot.requirementPresent || !snapshot.required {
			return true, false, fmt.Errorf("versioned stopped generation is missing required exit identity capability")
		}
		return true, true, nil
	}
	if snapshot.requirementPresent {
		return true, snapshot.required, nil
	}
	required, err = s.exitedIdentityRequiredUnlocked(id)
	if err != nil {
		return true, false, err
	}
	return true, required, nil
}

func (s *Store) clearExitedIdentityUnlocked(id string) error {
	if err := validateID(id); err != nil {
		return err
	}
	return removeStateFileDurable(s.ctrDir, exitedIdentityPath(s.ctrDir, id), "exited process identity")
}

func removeExitedIdentityForContainerState(containerStatePath string) error {
	if !strings.HasSuffix(containerStatePath, ".json") {
		return nil
	}
	base := strings.TrimSuffix(containerStatePath, ".json")
	for path, label := range map[string]string{
		base + exitedIdentitySuffix:         "exited process identity",
		base + exitedIdentityRequiredSuffix: "exited identity requirement",
	} {
		if err := os.Remove(path); err != nil && !os.IsNotExist(err) {
			return fmt.Errorf("remove %s: %w", label, err)
		}
	}
	return nil
}
