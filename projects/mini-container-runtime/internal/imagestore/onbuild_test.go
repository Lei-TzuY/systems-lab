package imagestore

import (
	"testing"
)

func TestExtractOnBuild(t *testing.T) {
	configJSON := []byte(`{
		"config": {
			"OnBuild": ["RUN npm install", "COPY . /app"]
		}
	}`)

	ob := ExtractOnBuild(configJSON)
	if len(ob) != 2 || ob[0] != "RUN npm install" {
		t.Fatalf("ExtractOnBuild = %v, want OnBuild triggers", ob)
	}
}
