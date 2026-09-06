package imagestore

import (
	"encoding/json"
)

type ConfigWithShell struct {
	Config struct {
		Shell []string `json:"Shell"`
	} `json:"config"`
}

// ExtractShell extracts config.Shell array from Image Config JSON.
func ExtractShell(configJSON []byte) []string {
	var cfg ConfigWithShell
	if err := json.Unmarshal(configJSON, &cfg); err == nil {
		if len(cfg.Config.Shell) > 0 {
			return cfg.Config.Shell
		}
	}
	return []string{"/bin/sh", "-c"}
}
