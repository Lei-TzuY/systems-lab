package imagestore

import (
	"strings"
)

const (
	MediaTypeOCIIndex     = "application/vnd.oci.image.index.v1+json"
	MediaTypeOCIManifest  = "application/vnd.oci.image.manifest.v1+json"
	MediaTypeDockerSchema2 = "application/vnd.docker.distribution.manifest.v2+json"
)

// ValidateMediaType checks if a media type string is a recognized OCI/Docker manifest type.
func ValidateMediaType(mediaType string) bool {
	return strings.HasPrefix(mediaType, "application/vnd.oci.") ||
		strings.HasPrefix(mediaType, "application/vnd.docker.")
}
