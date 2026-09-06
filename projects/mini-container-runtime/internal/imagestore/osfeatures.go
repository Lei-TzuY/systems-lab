package imagestore

import (
	"encoding/json"
)

type ConfigWithOSFeatures struct {
	OSFeatures []string `json:"os.features"`
}

// ExtractOSFeatures extracts os.features array from Image Config JSON.
func ExtractOSFeatures(configJSON []byte) []string {
	var cfg ConfigWithOSFeatures
	if err := json.Unmarshal(configJSON, &cfg); err == nil {
		return cfg.OSFeatures
	}
	return nil
}
