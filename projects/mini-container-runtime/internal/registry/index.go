package registry

import (
	"encoding/json"
)

type PlatformSpec struct {
	Architecture string `json:"architecture"`
	OS           string `json:"os"`
}

type ManifestDescriptor struct {
	MediaType string       `json:"mediaType"`
	Digest    string       `json:"digest"`
	Size      int64        `json:"size"`
	Platform  PlatformSpec `json:"platform"`
}

type OCIImageIndex struct {
	SchemaVersion int                  `json:"schemaVersion"`
	MediaType     string               `json:"mediaType"`
	Manifests     []ManifestDescriptor `json:"manifests"`
}

// BuildOCIImageIndex constructs a multi-arch OCI Image Index manifest JSON.
func BuildOCIImageIndex(manifests []ManifestDescriptor) ([]byte, error) {
	index := OCIImageIndex{
		SchemaVersion: 2,
		MediaType:     "application/vnd.oci.image.index.v1+json",
		Manifests:     manifests,
	}
	return json.MarshalIndent(index, "", "  ")
}
