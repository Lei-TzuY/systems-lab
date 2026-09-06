package events

import (
	"encoding/json"
	"fmt"
)

// MarshalJSON keeps event producers from persisting records that the reader
// deliberately rejects as semantically corrupt. This makes the append path and
// the replay path enforce the same event invariants instead of allowing a bad
// in-process caller to poison the durable audit stream for every later reader.
func (evt Event) MarshalJSON() ([]byte, error) {
	if err := validateEventRecord(evt); err != nil {
		return nil, fmt.Errorf("validate event: %w", err)
	}

	// Alias Event so json.Marshal does not recurse back into MarshalJSON.
	type eventJSON Event
	return json.Marshal(eventJSON(evt))
}
