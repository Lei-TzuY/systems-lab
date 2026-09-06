package state

import (
	"encoding/json"
	"fmt"
)

// UnmarshalJSON validates writer provenance before exposing persisted container
// state to callers. Records without state_schema_version remain readable as
// genuine pre-schema compatibility fixtures, while explicitly versioned state
// must match the schema understood by this runtime.
func (c *Container) UnmarshalJSON(data []byte) error {
	type containerAlias Container
	decoded := containerAlias{}
	envelope := struct {
		*containerAlias
		StateSchemaVersion json.RawMessage `json:"state_schema_version"`
	}{containerAlias: &decoded}

	if err := json.Unmarshal(data, &envelope); err != nil {
		return err
	}
	if len(envelope.StateSchemaVersion) != 0 {
		if string(envelope.StateSchemaVersion) == "null" {
			return fmt.Errorf("invalid container state schema version: null")
		}
		var version uint32
		if err := json.Unmarshal(envelope.StateSchemaVersion, &version); err != nil {
			return fmt.Errorf("unmarshal container state schema version: %w", err)
		}
		if version == 0 {
			return fmt.Errorf("invalid container state schema version 0")
		}
		if version != currentContainerStateSchemaVersion {
			return fmt.Errorf(
				"unsupported container state schema version %d (current %d)",
				version,
				currentContainerStateSchemaVersion,
			)
		}
	}

	*c = Container(decoded)
	return nil
}
