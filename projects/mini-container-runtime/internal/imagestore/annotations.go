package imagestore

import (
	"encoding/json"
	"fmt"
)

type AnnotatedManifest struct {
	Annotations map[string]string `json:"annotations"`
}

// ParseManifestAnnotations extracts key-value annotations from an OCI manifest JSON.
func ParseManifestAnnotations(manifestJSON []byte) (map[string]string, error) {
	var m AnnotatedManifest
	if err := json.Unmarshal(manifestJSON, &m); err != nil {
		return nil, fmt.Errorf("unmarshal annotations: %w", err)
	}

	if m.Annotations == nil {
		return make(map[string]string), nil
	}

	return m.Annotations, nil
}
