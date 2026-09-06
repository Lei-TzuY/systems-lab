package state

import (
	"encoding/json"
	"fmt"
	"path/filepath"
)

const currentStoppedGenerationSchemaVersion uint32 = 1

type stoppedGenerationTeardownSnapshot struct {
	status                     Status
	revision                   uint64
	stateVersion               uint32
	stateVersioned             bool
	version                    uint32
	versioned                  bool
	required                   bool
	requirementPresent         bool
	identity                   exitedIdentity
	identityEmbedded           bool
	legacyDNSCleanupAuthorized bool
	legacyDNSCleanupPresent    bool
}

type stoppedGenerationLifecycleSnapshot struct {
	status                     Status
	revision                   uint64
	stateVersion               json.RawMessage
	version                    json.RawMessage
	exitIdentityRequired       json.RawMessage
	exitIdentity               json.RawMessage
	legacyDNSCleanupAuthorized json.RawMessage
}

// readStoppedGenerationLifecycleSnapshotUnlocked reads the container lifecycle
// and teardown authority fields from exactly one container-state file snapshot.
// Teardown fields stay raw until the caller has established that status/revision
// still name the lifecycle generation it intends to act on.
func (s *Store) readStoppedGenerationLifecycleSnapshotUnlocked(id string) (stoppedGenerationLifecycleSnapshot, error) {
	if err := validateID(id); err != nil {
		return stoppedGenerationLifecycleSnapshot{}, err
	}
	path := filepath.Join(s.ctrDir, id+".json")
	data, err := readRegularStateFile(path, "container state")
	if err != nil {
		return stoppedGenerationLifecycleSnapshot{}, err
	}
	var metadata struct {
		Status                     Status          `json:"status"`
		Revision                   uint64          `json:"revision"`
		StateVersion               json.RawMessage `json:"state_schema_version"`
		Version                    json.RawMessage `json:"stopped_generation_schema_version"`
		ExitIdentityRequired       json.RawMessage `json:"exit_identity_required"`
		ExitIdentity               json.RawMessage `json:"exit_identity"`
		LegacyDNSCleanupAuthorized json.RawMessage `json:"legacy_dns_cleanup_authorized"`
	}
	if err := json.Unmarshal(data, &metadata); err != nil {
		return stoppedGenerationLifecycleSnapshot{}, fmt.Errorf("unmarshal stopped generation lifecycle snapshot: %w", err)
	}
	return stoppedGenerationLifecycleSnapshot{
		status:                     metadata.Status,
		revision:                   metadata.Revision,
		stateVersion:               metadata.StateVersion,
		version:                    metadata.Version,
		exitIdentityRequired:       metadata.ExitIdentityRequired,
		exitIdentity:               metadata.ExitIdentity,
		legacyDNSCleanupAuthorized: metadata.LegacyDNSCleanupAuthorized,
	}, nil
}

func (raw stoppedGenerationLifecycleSnapshot) teardownSnapshot() (stoppedGenerationTeardownSnapshot, error) {
	snapshot := stoppedGenerationTeardownSnapshot{
		status:   raw.status,
		revision: raw.revision,
	}

	if len(raw.stateVersion) != 0 {
		snapshot.stateVersioned = true
		if err := json.Unmarshal(raw.stateVersion, &snapshot.stateVersion); err != nil {
			return snapshot, fmt.Errorf("unmarshal container state schema version: %w", err)
		}
		if snapshot.stateVersion == 0 {
			return snapshot, fmt.Errorf("invalid container state schema version 0")
		}
		if snapshot.stateVersion != currentContainerStateSchemaVersion {
			return snapshot, fmt.Errorf("unsupported container state schema version %d (current %d)", snapshot.stateVersion, currentContainerStateSchemaVersion)
		}
	}

	if len(raw.version) != 0 {
		snapshot.versioned = true
		if err := json.Unmarshal(raw.version, &snapshot.version); err != nil {
			return snapshot, fmt.Errorf("unmarshal stopped generation schema version: %w", err)
		}
		if snapshot.version == 0 {
			return snapshot, fmt.Errorf("invalid stopped generation schema version 0")
		}
		if snapshot.version != currentStoppedGenerationSchemaVersion {
			return snapshot, fmt.Errorf(
				"unsupported stopped generation schema version %d (current %d)",
				snapshot.version,
				currentStoppedGenerationSchemaVersion,
			)
		}
	}

	if len(raw.exitIdentityRequired) != 0 {
		snapshot.requirementPresent = true
		if err := json.Unmarshal(raw.exitIdentityRequired, &snapshot.required); err != nil {
			return snapshot, fmt.Errorf("unmarshal stopped generation teardown metadata: unmarshal persisted exit identity requirement: %w", err)
		}
		if !snapshot.required {
			return snapshot, fmt.Errorf("invalid persisted exit identity requirement: false")
		}
	}

	if len(raw.exitIdentity) != 0 {
		if string(raw.exitIdentity) == "null" {
			return snapshot, fmt.Errorf("invalid persisted exit identity: null")
		}
		if err := json.Unmarshal(raw.exitIdentity, &snapshot.identity); err != nil {
			return snapshot, fmt.Errorf("unmarshal embedded exited process identity: %w", err)
		}
		if err := validateExitedIdentity(snapshot.identity); err != nil {
			return snapshot, err
		}
		snapshot.identityEmbedded = true
	}

	if len(raw.legacyDNSCleanupAuthorized) != 0 {
		snapshot.legacyDNSCleanupPresent = true
		if err := json.Unmarshal(raw.legacyDNSCleanupAuthorized, &snapshot.legacyDNSCleanupAuthorized); err != nil {
			return snapshot, fmt.Errorf("unmarshal legacy DNS cleanup authorization: %w", err)
		}
		if !snapshot.legacyDNSCleanupAuthorized {
			return snapshot, fmt.Errorf("invalid legacy DNS cleanup authorization: false")
		}
		if snapshot.versioned || snapshot.requirementPresent || snapshot.identityEmbedded {
			return snapshot, fmt.Errorf("legacy DNS cleanup authorization conflicts with exact stopped-generation metadata")
		}
	}
	return snapshot, nil
}

// readStoppedGenerationTeardownSnapshotUnlocked reads lifecycle coordinates,
// schema, capability, and exact identity from one container-state file snapshot.
func (s *Store) readStoppedGenerationTeardownSnapshotUnlocked(id string) (stoppedGenerationTeardownSnapshot, error) {
	raw, err := s.readStoppedGenerationLifecycleSnapshotUnlocked(id)
	if err != nil {
		return stoppedGenerationTeardownSnapshot{}, err
	}
	return raw.teardownSnapshot()
}

// migrateStoppedGenerationSnapshotUnlocked upgrades a valid unversioned exact
// stopped-generation identity in place without advancing the lifecycle revision.
// The caller holds the store lock and has already established that status/revision
// still identify the intended stopped generation, so the rewrite is a revision-CAS
// migration rather than a new lifecycle transition. Legacy .exit input is accepted
// only when its capability is authenticated by either the pre-version lifecycle
// JSON field or the older .exit-required marker. An orphan identity sidecar is
// ambiguous debris and fails closed instead of acquiring teardown authority.
func (s *Store) migrateStoppedGenerationSnapshotUnlocked(id string, revision uint64, snapshot stoppedGenerationTeardownSnapshot) (stoppedGenerationTeardownSnapshot, error) {
	if snapshot.versioned || snapshot.legacyDNSCleanupPresent {
		return snapshot, nil
	}
	if snapshot.stateVersioned {
		return snapshot, fmt.Errorf("container state schema version %d lacks explicit stopped-generation teardown authority", snapshot.stateVersion)
	}

	identity := snapshot.identity
	haveIdentity := snapshot.identityEmbedded
	if haveIdentity {
		if !snapshot.requirementPresent || !snapshot.required {
			return snapshot, fmt.Errorf("persisted exit identity exists without required lifecycle capability")
		}
	} else {
		legacyIdentity, ok, err := s.readExitedIdentityUnlocked(id)
		if err != nil {
			return snapshot, err
		}
		if !ok {
			return snapshot, nil
		}

		legacyAuthorized := snapshot.requirementPresent && snapshot.required
		if !legacyAuthorized {
			legacyAuthorized, err = s.exitedIdentityRequiredUnlocked(id)
			if err != nil {
				return snapshot, err
			}
		}
		if !legacyAuthorized {
			return snapshot, fmt.Errorf("legacy exited process identity exists without required capability marker")
		}
		identity = legacyIdentity
		haveIdentity = true
	}

	if !haveIdentity {
		return snapshot, nil
	}

	container, err := s.getUnlocked(id)
	if err != nil {
		return snapshot, err
	}
	if container.Status != StatusStopped || container.Revision != revision {
		return stoppedGenerationTeardownSnapshot{status: container.Status, revision: container.Revision}, nil
	}
	if err := s.writeContainerRevisionWithExitPolicyUnlocked(container, revision, true, &identity); err != nil {
		return snapshot, fmt.Errorf("migrate stopped generation teardown metadata: %w", err)
	}

	snapshot.stateVersion = currentContainerStateSchemaVersion
	snapshot.stateVersioned = true
	snapshot.version = currentStoppedGenerationSchemaVersion
	snapshot.versioned = true
	snapshot.required = true
	snapshot.requirementPresent = true
	snapshot.identity = identity
	snapshot.identityEmbedded = true
	return snapshot, nil
}

// authorizeLegacyDNSCleanupSnapshotUnlocked converts the one remaining implicit
// historical policy (no schema, capability, or exact identity) into an explicit,
// durable same-revision migration capability. Only records with no writer schema
// provenance are historical; modern writer output can never acquire broad cleanup
// authority merely because stopped-generation fields are absent.
func (s *Store) authorizeLegacyDNSCleanupSnapshotUnlocked(id string, revision uint64, snapshot stoppedGenerationTeardownSnapshot) (stoppedGenerationTeardownSnapshot, error) {
	if snapshot.versioned || snapshot.requirementPresent || snapshot.identityEmbedded || snapshot.legacyDNSCleanupPresent {
		return snapshot, nil
	}
	if snapshot.stateVersioned {
		return snapshot, fmt.Errorf("container state schema version %d lacks explicit stopped-generation teardown authority", snapshot.stateVersion)
	}

	required, err := s.exitedIdentityRequiredUnlocked(id)
	if err != nil {
		return snapshot, err
	}
	_, identityPresent, err := s.readExitedIdentityUnlocked(id)
	if err != nil {
		return snapshot, err
	}
	if required || identityPresent {
		return snapshot, nil
	}

	container, err := s.getUnlocked(id)
	if err != nil {
		return snapshot, err
	}
	if container.Status != StatusStopped || container.Revision != revision {
		return stoppedGenerationTeardownSnapshot{status: container.Status, revision: container.Revision}, nil
	}
	record := struct {
		*Container
		StateSchemaVersion             uint32 `json:"state_schema_version"`
		LegacyDNSCleanupAuthorized bool `json:"legacy_dns_cleanup_authorized"`
	}{Container: container, StateSchemaVersion: currentContainerStateSchemaVersion, LegacyDNSCleanupAuthorized: true}
	data, err := json.MarshalIndent(&record, "", "  ")
	if err != nil {
		return snapshot, fmt.Errorf("marshal legacy DNS cleanup migration state: %w", err)
	}
	path := filepath.Join(s.ctrDir, id+".json")
	if err := atomicWriteFile(s.ctrDir, path, data); err != nil {
		return snapshot, fmt.Errorf("persist legacy DNS cleanup migration state: %w", err)
	}

	snapshot.stateVersion = currentContainerStateSchemaVersion
	snapshot.stateVersioned = true
	snapshot.legacyDNSCleanupAuthorized = true
	snapshot.legacyDNSCleanupPresent = true
	return snapshot, nil
}

func (s *Store) validateLegacyDNSCleanupSidecarsUnlocked(id string, snapshot stoppedGenerationTeardownSnapshot) error {
	if !snapshot.legacyDNSCleanupPresent || !snapshot.legacyDNSCleanupAuthorized {
		return nil
	}
	required, err := s.exitedIdentityRequiredUnlocked(id)
	if err != nil {
		return err
	}
	_, identityPresent, err := s.readExitedIdentityUnlocked(id)
	if err != nil {
		return err
	}
	if required || identityPresent {
		return fmt.Errorf("legacy DNS cleanup authorization conflicts with exited identity sidecar metadata")
	}
	return nil
}

// readStoppedGenerationTeardownSnapshotForRevisionUnlocked first checks the
// caller's stopped revision against the same JSON snapshot that carries teardown
// authority. Stale callers return current=false without interpreting potentially
// malformed authority metadata from a newer generation. Valid legacy exact
// identities are upgraded once to the versioned embedded schema under the same
// lock. Pure pre-writer-schema records may be classified once into a durable
// same-revision legacy DNS migration capability. Once versioned lifecycle JSON
// is authoritative, legacy sidecars are durably retired; interrupted cleanup is
// safe and retried by later readers.
func (s *Store) readStoppedGenerationTeardownSnapshotForRevisionUnlocked(id string, revision uint64) (snapshot stoppedGenerationTeardownSnapshot, current bool, err error) {
	raw, err := s.readStoppedGenerationLifecycleSnapshotUnlocked(id)
	if err != nil {
		return stoppedGenerationTeardownSnapshot{}, false, err
	}
	if raw.status != StatusStopped || raw.revision != revision {
		return stoppedGenerationTeardownSnapshot{status: raw.status, revision: raw.revision}, false, nil
	}
	snapshot, err = raw.teardownSnapshot()
	if err != nil {
		return snapshot, true, err
	}
	if snapshot.versioned {
		if err := s.retireLegacyStoppedGenerationSidecarsUnlocked(id); err != nil {
			return snapshot, true, err
		}
		return snapshot, true, nil
	}
	if snapshot.legacyDNSCleanupPresent {
		if err := s.validateLegacyDNSCleanupSidecarsUnlocked(id, snapshot); err != nil {
			return snapshot, true, err
		}
		return snapshot, true, nil
	}

	migrated, err := s.migrateStoppedGenerationSnapshotUnlocked(id, revision, snapshot)
	if err != nil {
		return snapshot, true, err
	}
	if migrated.status != StatusStopped || migrated.revision != revision {
		return migrated, false, nil
	}
	if migrated.versioned {
		if err := s.retireLegacyStoppedGenerationSidecarsUnlocked(id); err != nil {
			return migrated, true, err
		}
		return migrated, true, nil
	}

	classified, err := s.authorizeLegacyDNSCleanupSnapshotUnlocked(id, revision, migrated)
	if err != nil {
		return migrated, true, err
	}
	if classified.status != StatusStopped || classified.revision != revision {
		return classified, false, nil
	}
	if classified.legacyDNSCleanupPresent {
		if err := s.validateLegacyDNSCleanupSidecarsUnlocked(id, classified); err != nil {
			return classified, true, err
		}
	}
	return classified, true, nil
}

// stoppedGenerationSchemaVersionUnlocked returns the explicitly persisted
// schema version for a modern stopped generation. Absence is reserved for
// upgrade compatibility with records written before schema versioning existed.
// Explicit zero and future versions are never interpreted as historical state.
func (s *Store) stoppedGenerationSchemaVersionUnlocked(id string) (version uint32, present bool, err error) {
	snapshot, err := s.readStoppedGenerationTeardownSnapshotUnlocked(id)
	if err != nil {
		return snapshot.version, snapshot.versioned, err
	}
	return snapshot.version, snapshot.versioned, nil
}
