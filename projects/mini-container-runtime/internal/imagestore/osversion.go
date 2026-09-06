package imagestore

import (
	"encoding/json"
)

type ConfigWithOSVersion struct {
	OS        string `json:"os"`
	OSVersion string `json:"os.version"`
}

// ExtractOSVersion extracts os.version string from Image Config JSON.
func ExtractOSVersion(configJSON []byte) string {
	var cfg ConfigWithOSVersion
	if err := json.Unmarshal(configJSON, &cfg); err == nil {
		return cfg.OSVersion
	}
	return ""
}
