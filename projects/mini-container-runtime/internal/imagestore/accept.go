package imagestore

import (
	"strings"
)

// BuildManifestAcceptHeader constructs the HTTP Accept header for OCI and Docker manifests.
func BuildManifestAcceptHeader() string {
	types := []string{
		"application/vnd.oci.image.index.v1+json",
		"application/vnd.oci.image.manifest.v1+json",
		"application/vnd.docker.distribution.manifest.v2+json",
		"application/vnd.docker.distribution.manifest.list.v2+json",
	}
	return strings.Join(types, ", ")
}
