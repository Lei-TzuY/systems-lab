package imagestore

import (
	"encoding/json"
)

type ConfigStopSignal struct {
	Config struct {
		StopSignal string `json:"StopSignal"`
	} `json:"config"`
	StopSignal string `json:"stopSignal"`
}

// ExtractStopSignal extracts stopSignal field from Image Config JSON.
func ExtractStopSignal(configJSON []byte) string {
	var cfg ConfigStopSignal
	if err := json.Unmarshal(configJSON, &cfg); err == nil {
		if cfg.Config.StopSignal != "" {
			return cfg.Config.StopSignal
		}
		if cfg.StopSignal != "" {
			return cfg.StopSignal
		}
	}
	return "SIGTERM"
}
