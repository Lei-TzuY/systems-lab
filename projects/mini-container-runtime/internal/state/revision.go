package state

import (
	"encoding/json"
	"errors"
	"fmt"
	"math"
	"os"
	"path/filepath"
)

const currentContainerStateSchemaVersion uint32 = 1

// ErrRevisionConflict means the caller is trying to persist a stale container
// snapshot. Callers must reload the current record instead of overwriting a
// newer lifecycle transition or recreating a record that was deleted.
var ErrRevisionConflict = errors.New("container state revision conflict")

func (s *Store) saveContainerCASUnlocked(c *Container) error {
	if c == nil {
		return fmt.Errorf("container state is nil")
	}
	target := filepath.Join(s.ctrDir, c.ID+".json")
	nextRevision := uint64(1)

	data, err := readRegularStateFile(target, "container state")
	switch {
	case err == nil:
		var current Container
		if err := unmarshalContainerStateForID(data, c.ID, &current); err != nil {
			return fmt.Errorf("unmarshal current container state: %w", err)
		}
		if current.Revision != c.Revision {
			return fmt.Errorf("%w for %s: caller=%d current=%d", ErrRevisionConflict, c.ID, c.Revision, current.Revision)
		}
		if current.Revision == math.MaxUint64 {
			return fmt.Errorf("container %s revision overflow", c.ID)
		}
		nextRevision = current.Revision + 1
	case errors.Is(err, os.ErrNotExist):
		if c.Revision != 0 {
			return fmt.Errorf("%w for %s: record was deleted after revision %d", ErrRevisionConflict, c.ID, c.Revision)
		}
	default:
		return fmt.Errorf("read current container state: %w", err)
	}

	return s.writeContainerRevisionUnlocked(c, nextRevision)
}

func (s *Store) writeContainerNextRevisionUnlocked(c *Container) error {
	if c.Revision == math.MaxUint64 {
		return fmt.Errorf("container %s revision overflow", c.ID)
	}
	return s.writeContainerRevisionUnlocked(c, c.Revision+1)
}

// writeStoppedContainerNextRevisionUnlocked publishes stopped status, revision,
// capability, schema version, and exact generation identity in one atomic JSON
// replacement.
func (s *Store) writeStoppedContainerNextRevisionUnlocked(c *Container, pid int, pidStartTime uint64) error {
	if c.Revision == math.MaxUint64 {
		return fmt.Errorf("container %s revision overflow", c.ID)
	}
	identity := exitedIdentity{PID: pid, PIDStartTime: pidStartTime}
	if err := validateExitedIdentity(identity); err != nil {
		return err
	}
	return s.writeContainerRevisionWithExitPolicyUnlocked(c, c.Revision+1, true, &identity)
}

func (s *Store) writeContainerRevisionUnlocked(c *Container, revision uint64) error {
	// Lifecycle-only metadata is preserved only while the target remains stopped.
	// A transition to running intentionally drops it so stale stopped-generation
	// authority cannot leak into a later generation.
	if c.Status != StatusStopped {
		return s.writeContainerRevisionWithExitPolicyUnlocked(c, revision, false, nil)
	}

	required, present, err := s.containerExitIdentityRequirementUnlocked(c.ID)
	if err != nil && !errors.Is(err, os.ErrNotExist) {
		return err
	}
	identity, identityPresent, err := s.containerEmbeddedExitedIdentityUnlocked(c.ID)
	if err != nil && !errors.Is(err, os.ErrNotExist) {
		return err
	}
	if identityPresent && (!present || !required) {
		return fmt.Errorf("persisted exit identity exists without required lifecycle capability")
	}
	if identityPresent {
		return s.writeContainerRevisionWithExitPolicyUnlocked(c, revision, true, &identity)
	}
	return s.writeContainerRevisionWithExitPolicyUnlocked(c, revision, present && required, nil)
}

func (s *Store) writeContainerRevisionWithExitPolicyUnlocked(c *Container, revision uint64, requireExitIdentity bool, identity *exitedIdentity) error {
	copy := *c
	copy.Revision = revision

	if identity != nil {
		if !requireExitIdentity {
			return fmt.Errorf("exit identity requires lifecycle capability")
		}
		if err := validateExitedIdentity(*identity); err != nil {
			return err
		}
	}

	var data []byte
	var err error
	switch {
	case requireExitIdentity && identity != nil:
		record := struct {
			*Container
			StateSchemaVersion             uint32          `json:"state_schema_version"`
			StoppedGenerationSchemaVersion uint32          `json:"stopped_generation_schema_version"`
			ExitIdentityRequired           bool            `json:"exit_identity_required"`
			ExitIdentity                   *exitedIdentity `json:"exit_identity"`
		}{
			Container:                      &copy,
			StateSchemaVersion:             currentContainerStateSchemaVersion,
			StoppedGenerationSchemaVersion: currentStoppedGenerationSchemaVersion,
			ExitIdentityRequired:           true,
			ExitIdentity:                   identity,
		}
		data, err = json.MarshalIndent(&record, "", "  ")
	case requireExitIdentity:
		// A capability without an embedded identity exists only as an upgrade/test
		// fixture for the historical capability+sidecar format. Do not stamp modern
		// writer provenance onto that deliberately pre-schema representation.
		record := struct {
			*Container
			ExitIdentityRequired bool `json:"exit_identity_required"`
		}{Container: &copy, ExitIdentityRequired: true}
		data, err = json.MarshalIndent(&record, "", "  ")
	case copy.Status == StatusStopped:
		// A current writer that persists a stopped record without exact process
		// identity must make its broad cleanup policy explicit. Field absence is
		// reserved for genuinely pre-schema historical state.
		record := struct {
			*Container
			StateSchemaVersion             uint32 `json:"state_schema_version"`
			LegacyDNSCleanupAuthorized bool   `json:"legacy_dns_cleanup_authorized"`
		}{
			Container:                      &copy,
			StateSchemaVersion:             currentContainerStateSchemaVersion,
			LegacyDNSCleanupAuthorized:     true,
		}
		data, err = json.MarshalIndent(&record, "", "  ")
	default:
		record := struct {
			*Container
			StateSchemaVersion uint32 `json:"state_schema_version"`
		}{Container: &copy, StateSchemaVersion: currentContainerStateSchemaVersion}
		data, err = json.MarshalIndent(&record, "", "  ")
	}
	if err != nil {
		return fmt.Errorf("marshal container: %w", err)
	}
	if err := validateStateFileWrite(data, "container state"); err != nil {
		return err
	}
	target := filepath.Join(s.ctrDir, c.ID+".json")
	if err := atomicWriteFile(s.ctrDir, target, data); err != nil {
		return err
	}
	// Only publish the new revision to the caller after durable file creation,
	// rename, and parent-directory sync have succeeded. Failed writes leave the
	// caller's CAS token intact.
	c.Revision = revision
	return nil
}
