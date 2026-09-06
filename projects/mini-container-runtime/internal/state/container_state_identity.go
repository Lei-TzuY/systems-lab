package state

import (
	"encoding/json"
	"fmt"
)

// unmarshalContainerStateForID decodes one persisted container record and binds
// its payload identity to the storage key selected by the caller. The filename
// is the authoritative lookup boundary; payload data must never redirect a read
// or read-modify-write operation to a different logical container.
func unmarshalContainerStateForID(data []byte, expectedID string, dst *Container) error {
	if err := json.Unmarshal(data, dst); err != nil {
		return err
	}
	if dst.ID != expectedID {
		return fmt.Errorf(
			"container state identity mismatch: record %q contains id %q",
			expectedID,
			dst.ID,
		)
	}
	return nil
}
