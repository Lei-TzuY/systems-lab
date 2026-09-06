package imagestore

import (
	"testing"
)

func TestExtractArtifactType(t *testing.T) {
	manifestJSON := []byte(`{
		"schemaVersion": 2,
		"artifactType": "application/vnd.cncf.helm.chart.content.v1.tar+gzip"
	}`)

	artType := ExtractArtifactType(manifestJSON)
	if artType != "application/vnd.cncf.helm.chart.content.v1.tar+gzip" {
		t.Fatalf("ExtractArtifactType = %s, want Helm chart artifactType", artType)
	}
}
