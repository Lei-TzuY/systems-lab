package imagestore

import (
	"encoding/json"
	"fmt"
)

type SubjectDescriptor struct {
	MediaType string `json:"mediaType"`
	Digest    string `json:"digest"`
	Size      int64  `json:"size"`
}

type ManifestWithSubject struct {
	Subject *SubjectDescriptor `json:"subject,omitempty"`
}

// ExtractManifestSubject extracts optional OCI subject descriptor from manifest JSON.
func ExtractManifestSubject(manifestJSON []byte) (*SubjectDescriptor, error) {
	var m ManifestWithSubject
	if err := json.Unmarshal(manifestJSON, &m); err != nil {
		return nil, fmt.Errorf("unmarshal subject manifest: %w", err)
	}

	return m.Subject, nil
}
