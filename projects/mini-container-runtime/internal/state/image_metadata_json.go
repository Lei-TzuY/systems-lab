package state

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
)

// decodeImageMetadataStrict rejects ambiguous or forward-incompatible persisted
// metadata before it can influence image identity, cleanup, or migration. JSON's
// default decoder silently accepts duplicate object keys and unknown struct
// fields, which makes the authoritative interpretation depend on decoder
// behavior rather than the bytes on disk.
func decodeImageMetadataStrict(data []byte, img *Image) error {
	if img == nil {
		return fmt.Errorf("image metadata destination is nil")
	}
	if err := validateUniqueJSONKeys(data); err != nil {
		return err
	}

	dec := json.NewDecoder(bytes.NewReader(data))
	dec.DisallowUnknownFields()
	if err := dec.Decode(img); err != nil {
		return err
	}
	if err := requireJSONEOF(dec); err != nil {
		return err
	}
	return nil
}

func requireJSONEOF(dec *json.Decoder) error {
	var trailing any
	if err := dec.Decode(&trailing); err != io.EOF {
		if err == nil {
			return fmt.Errorf("unexpected trailing JSON value")
		}
		return fmt.Errorf("decode trailing JSON: %w", err)
	}
	return nil
}

// validateUniqueJSONKeys walks the complete JSON value and rejects duplicate
// keys at every object depth. This is deliberately independent of Image's
// current schema so nested objects introduced later cannot inherit last-key-wins
// ambiguity by accident.
func validateUniqueJSONKeys(data []byte) error {
	dec := json.NewDecoder(bytes.NewReader(data))
	var walkValue func() error
	walkValue = func() error {
		tok, err := dec.Token()
		if err != nil {
			return err
		}
		delim, ok := tok.(json.Delim)
		if !ok {
			return nil
		}

		switch delim {
		case '{':
			seen := make(map[string]struct{})
			for dec.More() {
				keyToken, err := dec.Token()
				if err != nil {
					return err
				}
				key, ok := keyToken.(string)
				if !ok {
					return fmt.Errorf("invalid JSON object key %v", keyToken)
				}
				if _, exists := seen[key]; exists {
					return fmt.Errorf("duplicate JSON object key %q", key)
				}
				seen[key] = struct{}{}
				if err := walkValue(); err != nil {
					return err
				}
			}
			end, err := dec.Token()
			if err != nil {
				return err
			}
			if end != json.Delim('}') {
				return fmt.Errorf("invalid JSON object terminator %v", end)
			}
		case '[':
			for dec.More() {
				if err := walkValue(); err != nil {
					return err
				}
			}
			end, err := dec.Token()
			if err != nil {
				return err
			}
			if end != json.Delim(']') {
				return fmt.Errorf("invalid JSON array terminator %v", end)
			}
		default:
			return fmt.Errorf("unexpected JSON delimiter %q", delim)
		}
		return nil
	}

	if err := walkValue(); err != nil {
		return err
	}
	if _, err := dec.Token(); err != io.EOF {
		if err == nil {
			return fmt.Errorf("unexpected trailing JSON value")
		}
		return fmt.Errorf("decode trailing JSON: %w", err)
	}
	return nil
}
