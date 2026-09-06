package imagestore

import (
	"encoding/json"
	"fmt"
)

type SizedDescriptor struct {
	Size int64 `json:"size"`
}

type SizedManifest struct {
	Config SizedDescriptor   `json:"config"`
	Layers []SizedDescriptor `json:"layers"`
}

// CalculateManifestTotalSize sums the bytes of config descriptor and layer descriptors.
func CalculateManifestTotalSize(manifestJSON []byte) (int64, error) {
	var m SizedManifest
	if err := json.Unmarshal(manifestJSON, &m); err != nil {
		return 0, fmt.Errorf("unmarshal manifest descriptor: %w", err)
	}

	var total int64 = m.Config.Size
	for _, layer := range m.Layers {
		total += layer.Size
	}

	return total, nil
}
