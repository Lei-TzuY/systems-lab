package registry

import (
	"strings"
	"testing"
)

func TestOCIImageIndex(t *testing.T) {
	manifests := []ManifestDescriptor{
		{
			MediaType: "application/vnd.oci.image.manifest.v1+json",
			Digest:    "sha256:1111111111111111111111111111111111111111111111111111111111111111",
			Size:      1234,
			Platform:  PlatformSpec{Architecture: "amd64", OS: "linux"},
		},
		{
			MediaType: "application/vnd.oci.image.manifest.v1+json",
			Digest:    "sha256:2222222222222222222222222222222222222222222222222222222222222222",
			Size:      1234,
			Platform:  PlatformSpec{Architecture: "arm64", OS: "linux"},
		},
	}

	data, err := BuildOCIImageIndex(manifests)
	if err != nil {
		t.Fatalf("BuildOCIImageIndex error: %v", err)
	}

	strData := string(data)
	if !strings.Contains(strData, "application/vnd.oci.image.index.v1+json") {
		t.Fatalf("Index missing mediaType:\n%s", strData)
	}
	if !strings.Contains(strData, "arm64") || !strings.Contains(strData, "amd64") {
		t.Fatalf("Index missing platform descriptors:\n%s", strData)
	}
}
