package imagestore

import (
	"encoding/json"
	"fmt"
)

type Schema2Descriptor struct {
	MediaType string `json:"mediaType"`
	Digest    string `json:"digest"`
	Size      int64  `json:"size"`
}

type Schema2Manifest struct {
	SchemaVersion int                 `json:"schemaVersion"`
	MediaType     string              `json:"mediaType"`
	Config        Schema2Descriptor   `json:"config"`
	Layers        []Schema2Descriptor `json:"layers"`
}

// ValidateSchema2Manifest validates an OCI / Docker Schema v2 Manifest structure.
func ValidateSchema2Manifest(manifestJSON []byte) (bool, error) {
	var m Schema2Manifest
	if err := json.Unmarshal(manifestJSON, &m); err != nil {
		return false, fmt.Errorf("unmarshal schema2 manifest: %w", err)
	}

	if m.SchemaVersion != 2 {
		return false, fmt.Errorf("invalid schemaVersion: %d (want 2)", m.SchemaVersion)
	}

	return true, nil
}
