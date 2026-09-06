package imagestore

import (
	"encoding/json"
	"fmt"
)

type ConfigWithContainerConfig struct {
	ContainerConfig map[string]interface{} `json:"container_config"`
}

// ExtractContainerConfig extracts container_config struct from Image Config JSON.
func ExtractContainerConfig(configJSON []byte) (map[string]interface{}, error) {
	var cfg ConfigWithContainerConfig
	if err := json.Unmarshal(configJSON, &cfg); err != nil {
		return nil, fmt.Errorf("unmarshal container_config: %w", err)
	}
	return cfg.ContainerConfig, nil
}
