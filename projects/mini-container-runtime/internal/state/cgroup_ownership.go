package state

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
)

const (
	cgroupOwnershipSuffix        = ".cgroup"
	cgroupOwnershipSchemaVersion = 1
)

// CgroupOwnership is a runtime-private durable proof that one exact process
// generation owns a cgroup created by minicontainer. The sidecar intentionally
// survives the running -> stopped transition until cgroup cleanup succeeds.
//
// Keeping this proof outside Container JSON prevents ordinary lifecycle Save
// calls from accidentally erasing cleanup state and lets stopped records retain
// the PID/start-time generation even after their public PID fields are cleared.
type CgroupOwnership struct {
	Name         string `json:"name"`
	PID          int    `json:"pid"`
	PIDStartTime uint64 `json:"pid_start_time"`
}

type persistedCgroupOwnership struct {
	SchemaVersion int    `json:"schema_version"`
	ContainerID   string `json:"container_id"`
	Name          string `json:"name"`
	PID           int    `json:"pid"`
	PIDStartTime  uint64 `json:"pid_start_time"`
}

type cgroupOwnershipEnvelope struct {
	SchemaVersion json.RawMessage `json:"schema_version"`
	ContainerID   json.RawMessage `json:"container_id"`
	Name          string          `json:"name"`
	PID           int             `json:"pid"`
	PIDStartTime  uint64          `json:"pid_start_time"`
}

func cgroupOwnershipPath(containerDir, id string) string {
	return filepath.Join(containerDir, id+cgroupOwnershipSuffix)
}

func validatePersistedCgroupName(name string) error {
	if name == "" {
		return fmt.Errorf("cgroup name must not be empty")
	}
	if len(name) > 255 {
		return fmt.Errorf("cgroup name exceeds 255 bytes")
	}
	for i := 0; i < len(name); i++ {
		c := name[i]
		valid := (c >= 'a' && c <= 'z') ||
			(c >= 'A' && c <= 'Z') ||
			(c >= '0' && c <= '9') ||
			c == '-' || c == '_' || c == '.'
		if !valid {
			return fmt.Errorf("cgroup name %q contains invalid character %q", name, c)
		}
	}
	if name == "." || name == ".." {
		return fmt.Errorf("invalid cgroup name %q", name)
	}
	return nil
}

func validateCgroupOwnership(o CgroupOwnership) error {
	if err := validatePersistedCgroupName(o.Name); err != nil {
		return err
	}
	if o.PID <= 0 || o.PIDStartTime == 0 {
		return fmt.Errorf("invalid cgroup process identity %d/%d", o.PID, o.PIDStartTime)
	}
	return nil
}

func (s *Store) writeCgroupOwnershipUnlocked(id string, ownership CgroupOwnership) error {
	if err := validateID(id); err != nil {
		return err
	}
	if err := validateCgroupOwnership(ownership); err != nil {
		return err
	}
	persisted := persistedCgroupOwnership{
		SchemaVersion: cgroupOwnershipSchemaVersion,
		ContainerID:   id,
		Name:          ownership.Name,
		PID:           ownership.PID,
		PIDStartTime:  ownership.PIDStartTime,
	}
	data, err := json.Marshal(persisted)
	if err != nil {
		return fmt.Errorf("marshal cgroup ownership: %w", err)
	}
	return atomicWriteFile(s.ctrDir, cgroupOwnershipPath(s.ctrDir, id), data)
}

func decodeCgroupOwnership(id string, data []byte) (CgroupOwnership, error) {
	var envelope cgroupOwnershipEnvelope
	if err := json.Unmarshal(data, &envelope); err != nil {
		return CgroupOwnership{}, fmt.Errorf("unmarshal cgroup ownership: %w", err)
	}

	// Pre-schema sidecars are historical cleanup evidence. Keep them readable so
	// upgrades do not strand pending cgroups, but once either provenance field is
	// present the record must satisfy the complete current envelope. This prevents
	// malformed or partially downgraded records from silently regaining legacy
	// authority.
	legacy := envelope.SchemaVersion == nil && envelope.ContainerID == nil
	if !legacy {
		if envelope.SchemaVersion == nil || envelope.ContainerID == nil {
			return CgroupOwnership{}, fmt.Errorf("invalid persisted cgroup ownership provenance: schema_version and container_id must both be present")
		}
		var schemaVersion int
		if err := json.Unmarshal(envelope.SchemaVersion, &schemaVersion); err != nil {
			return CgroupOwnership{}, fmt.Errorf("invalid persisted cgroup ownership schema_version: %w", err)
		}
		if schemaVersion != cgroupOwnershipSchemaVersion {
			return CgroupOwnership{}, fmt.Errorf("unsupported persisted cgroup ownership schema_version %d", schemaVersion)
		}
		var containerID string
		if err := json.Unmarshal(envelope.ContainerID, &containerID); err != nil {
			return CgroupOwnership{}, fmt.Errorf("invalid persisted cgroup ownership container_id: %w", err)
		}
		if err := validateID(containerID); err != nil {
			return CgroupOwnership{}, fmt.Errorf("invalid persisted cgroup ownership container_id: %w", err)
		}
		if containerID != id {
			return CgroupOwnership{}, fmt.Errorf("cgroup ownership storage key %q does not match persisted container_id %q", id, containerID)
		}
	}

	ownership := CgroupOwnership{
		Name:         envelope.Name,
		PID:          envelope.PID,
		PIDStartTime: envelope.PIDStartTime,
	}
	if err := validateCgroupOwnership(ownership); err != nil {
		return CgroupOwnership{}, fmt.Errorf("invalid persisted cgroup ownership: %w", err)
	}
	return ownership, nil
}

func (s *Store) readCgroupOwnershipUnlocked(id string) (CgroupOwnership, bool, error) {
	if err := validateID(id); err != nil {
		return CgroupOwnership{}, false, err
	}
	path := cgroupOwnershipPath(s.ctrDir, id)
	data, err := readRegularStateFile(path, "cgroup ownership")
	if err != nil {
		if os.IsNotExist(err) {
			return CgroupOwnership{}, false, nil
		}
		return CgroupOwnership{}, false, fmt.Errorf("read cgroup ownership: %w", err)
	}
	ownership, err := decodeCgroupOwnership(id, data)
	if err != nil {
		return CgroupOwnership{}, false, err
	}
	return ownership, true, nil
}

func (s *Store) clearCgroupOwnershipUnlocked(id string) error {
	if err := validateID(id); err != nil {
		return err
	}
	return removeStateFileDurable(s.ctrDir, cgroupOwnershipPath(s.ctrDir, id), "cgroup ownership")
}

// GetCgroupOwnership reads the durable cgroup ownership proof for a container.
// Missing sidecars are expected for legacy state and for generations whose
// cgroup setup never succeeded.
func (s *Store) GetCgroupOwnership(id string) (CgroupOwnership, bool, error) {
	if s == nil {
		return CgroupOwnership{}, false, fmt.Errorf("state store is nil")
	}
	if err := validateID(id); err != nil {
		return CgroupOwnership{}, false, err
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	if s.lockFile == nil {
		return CgroupOwnership{}, false, ErrStoreClosed
	}
	return s.readCgroupOwnershipUnlocked(id)
}

// MarkCgroupOwnedIfIdentity durably records cgroup ownership only while the
// container still points at the exact process identity that was admitted to the
// cgroup. Repeating the same ownership write is idempotent; conflicting
// ownership is rejected instead of overwriting cleanup responsibility.
func (s *Store) MarkCgroupOwnedIfIdentity(id string, pid int, pidStartTime uint64, name string) error {
	if s == nil {
		return fmt.Errorf("state store is nil")
	}
	if err := validateID(id); err != nil {
		return err
	}
	ownership := CgroupOwnership{Name: name, PID: pid, PIDStartTime: pidStartTime}
	if err := validateCgroupOwnership(ownership); err != nil {
		return err
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	if err := lockStateFile(s.lockFile); err != nil {
		return err
	}
	defer func() { _ = unlockStateFile(s.lockFile) }()

	c, err := s.getUnlocked(id)
	if err != nil {
		return err
	}
	if c.Status != StatusRunning || c.PID != pid || c.PIDStartTime != pidStartTime {
		return fmt.Errorf("container %s is not bound to process %d/%d while recording cgroup ownership", id, pid, pidStartTime)
	}

	existing, ok, err := s.readCgroupOwnershipUnlocked(id)
	if err != nil {
		return err
	}
	if ok {
		if existing == ownership {
			return nil
		}
		return fmt.Errorf("container %s already has pending cgroup ownership for %s (%d/%d)", id, existing.Name, existing.PID, existing.PIDStartTime)
	}
	return s.writeCgroupOwnershipUnlocked(id, ownership)
}

// ClearCgroupOwnershipIfMatch removes one exact ownership proof after cleanup
// succeeds. It refuses to clear ownership while the container record is still
// running, and a stale actor cannot clear a different generation's sidecar.
func (s *Store) ClearCgroupOwnershipIfMatch(id string, ownership CgroupOwnership) (bool, error) {
	if s == nil {
		return false, fmt.Errorf("state store is nil")
	}
	if err := validateID(id); err != nil {
		return false, err
	}
	if err := validateCgroupOwnership(ownership); err != nil {
		return false, err
	}

	s.mu.Lock()
	defer s.mu.Unlock()
	if err := lockStateFile(s.lockFile); err != nil {
		return false, err
	}
	defer func() { _ = unlockStateFile(s.lockFile) }()

	existing, ok, err := s.readCgroupOwnershipUnlocked(id)
	if err != nil {
		return false, err
	}
	if !ok || existing != ownership {
		return false, nil
	}

	c, err := s.getUnlocked(id)
	if err != nil {
		return false, err
	}
	if c.Status == StatusRunning {
		return false, fmt.Errorf("refusing to clear cgroup ownership for running container %s", id)
	}
	if err := s.clearCgroupOwnershipUnlocked(id); err != nil {
		return false, err
	}
	return true, nil
}
