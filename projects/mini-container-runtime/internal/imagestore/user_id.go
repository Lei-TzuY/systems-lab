package imagestore

import (
	"encoding/json"
)

type ConfigContainerUser struct {
	Config struct {
		User string `json:"User"`
	} `json:"config"`
	User string `json:"user"`
}

// ExtractImageUser extracts user field from Image Config JSON.
func ExtractImageUser(configJSON []byte) string {
	var cfg ConfigContainerUser
	if err := json.Unmarshal(configJSON, &cfg); err == nil {
		if cfg.Config.User != "" {
			return cfg.Config.User
		}
		if cfg.User != "" {
			return cfg.User
		}
	}
	return "root"
}
