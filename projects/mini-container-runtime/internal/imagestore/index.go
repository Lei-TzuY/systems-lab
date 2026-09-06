package imagestore

import (
	"encoding/json"
	"fmt"
)

type IndexPlatform struct {
	Architecture string `json:"architecture"`
	OS           string `json:"os"`
}

type IndexManifestDescriptor struct {
	MediaType string        `json:"mediaType"`
	Digest    string        `json:"digest"`
	Size      int64         `json:"size"`
	Platform  IndexPlatform `json:"platform"`
}

type OCIIndex struct {
	SchemaVersion int                       `json:"schemaVersion"`
	MediaType     string                    `json:"mediaType"`
	Manifests     []IndexManifestDescriptor `json:"manifests"`
}

// ResolveManifestIndex parses an OCI Index JSON and resolves target OS/Arch digest.
func ResolveManifestIndex(indexJSON []byte, targetOS, targetArch string) (string, error) {
	var idx OCIIndex
	if err := json.Unmarshal(indexJSON, &idx); err != nil {
		return "", fmt.Errorf("unmarshal index json: %w", err)
	}

	for _, m := range idx.Manifests {
		if m.Platform.OS == targetOS && m.Platform.Architecture == targetArch {
			return m.Digest, nil
		}
	}

	if len(idx.Manifests) > 0 {
		return idx.Manifests[0].Digest, nil
	}

	return "", fmt.Errorf("no matching manifest found for %s/%s", targetOS, targetArch)
}
