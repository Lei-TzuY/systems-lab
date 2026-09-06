package imagestore

import (
	"encoding/json"
)

type ConfigWithOnBuild struct {
	Config struct {
		OnBuild []string `json:"OnBuild"`
	} `json:"config"`
	OnBuild []string `json:"onbuild"`
}

// ExtractOnBuild extracts OnBuild array from Image Config JSON.
func ExtractOnBuild(configJSON []byte) []string {
	var cfg ConfigWithOnBuild
	if err := json.Unmarshal(configJSON, &cfg); err == nil {
		if len(cfg.Config.OnBuild) > 0 {
			return cfg.Config.OnBuild
		}
		if len(cfg.OnBuild) > 0 {
			return cfg.OnBuild
		}
	}
	return nil
}
