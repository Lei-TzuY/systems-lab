package dns

import (
	"encoding/json"
	"fmt"
)

const currentHostEntrySchemaVersion = 1

// MarshalJSON stamps every host entry written by the current runtime with an
// explicit schema version. Container and generation provenance remain part of
// the entry payload itself, so readers can distinguish current records from
// genuinely pre-schema historical records without changing the on-disk
// registry shape.
func (entry HostEntry) MarshalJSON() ([]byte, error) {
	type hostEntryAlias HostEntry
	return json.Marshal(struct {
		SchemaVersion int `json:"schema_version"`
		hostEntryAlias
	}{
		SchemaVersion:  currentHostEntrySchemaVersion,
		hostEntryAlias: hostEntryAlias(entry),
	})
}

// UnmarshalJSON accepts records with no schema_version only as historical
// pre-schema input. Once schema provenance is present it is authoritative:
// malformed, zero, or future versions fail closed instead of being silently
// reinterpreted as legacy ownership evidence.
func (entry *HostEntry) UnmarshalJSON(data []byte) error {
	var envelope map[string]json.RawMessage
	if err := json.Unmarshal(data, &envelope); err != nil {
		return err
	}
	if rawVersion, ok := envelope["schema_version"]; ok {
		var version int
		if err := json.Unmarshal(rawVersion, &version); err != nil {
			return fmt.Errorf("invalid DNS host entry schema version: %w", err)
		}
		if version != currentHostEntrySchemaVersion {
			return fmt.Errorf("unsupported DNS host entry schema version %d", version)
		}
	}

	type hostEntryAlias HostEntry
	var decoded hostEntryAlias
	if err := json.Unmarshal(data, &decoded); err != nil {
		return err
	}
	*entry = HostEntry(decoded)
	return nil
}
