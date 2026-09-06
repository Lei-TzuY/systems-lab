package imagestore

import (
	"encoding/json"
)

type ConfigWithVariant struct {
	Architecture string `json:"architecture"`
	Variant      string `json:"variant"`
}

// ExtractImageVariant extracts architecture variant (e.g. v7) from Image Config JSON.
func ExtractImageVariant(configJSON []byte) string {
	var cfg ConfigWithVariant
	if err := json.Unmarshal(configJSON, &cfg); err == nil {
		return cfg.Variant
	}
	return ""
}
