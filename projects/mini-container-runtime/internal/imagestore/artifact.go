package imagestore

import (
	"encoding/json"
)

type ManifestWithArtifact struct {
	ArtifactType string `json:"artifactType"`
	MediaType    string `json:"mediaType"`
}

// ExtractArtifactType extracts the artifactType or mediaType string from manifest JSON.
func ExtractArtifactType(manifestJSON []byte) string {
	var m ManifestWithArtifact
	if err := json.Unmarshal(manifestJSON, &m); err == nil {
		if m.ArtifactType != "" {
			return m.ArtifactType
		}
		if m.MediaType != "" {
			return m.MediaType
		}
	}
	return "application/vnd.oci.image.manifest.v1+json"
}
