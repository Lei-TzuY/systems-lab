package events

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
)

// rejectDuplicateTopLevelFields rejects ambiguous audit records before normal
// decoding. encoding/json otherwise accepts duplicate object keys and silently
// keeps the last value, which lets a corrupted or tampered record redefine
// security-relevant fields such as type or container_id without being noticed.
func rejectDuplicateTopLevelFields(line []byte) error {
	dec := json.NewDecoder(bytes.NewReader(line))

	tok, err := dec.Token()
	if err != nil {
		return err
	}
	open, ok := tok.(json.Delim)
	if !ok || open != '{' {
		return fmt.Errorf("event record must be a JSON object")
	}

	seen := make(map[string]struct{})
	for dec.More() {
		keyToken, err := dec.Token()
		if err != nil {
			return err
		}
		key, ok := keyToken.(string)
		if !ok {
			return fmt.Errorf("event record contains a non-string field name")
		}
		if _, exists := seen[key]; exists {
			return fmt.Errorf("duplicate field %q", key)
		}
		seen[key] = struct{}{}

		var value json.RawMessage
		if err := dec.Decode(&value); err != nil {
			return err
		}
	}

	tok, err = dec.Token()
	if err != nil {
		return err
	}
	closeDelim, ok := tok.(json.Delim)
	if !ok || closeDelim != '}' {
		return fmt.Errorf("event record has invalid object termination")
	}

	// Reject a second top-level JSON value while permitting trailing whitespace.
	var extra json.RawMessage
	if err := dec.Decode(&extra); err != io.EOF {
		if err == nil {
			return fmt.Errorf("event record contains trailing JSON data")
		}
		return err
	}
	return nil
}
