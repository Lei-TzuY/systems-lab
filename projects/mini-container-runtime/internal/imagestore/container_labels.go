package imagestore

import (
	"encoding/json"
	"fmt"
)

type ConfigWithLabels struct {
	Config struct {
		Labels map[string]string `json:"Labels"`
	} `json:"config"`
}

// ExtractLabels extracts config.Labels map from Image Config JSON.
func ExtractLabels(configJSON []byte) (map[string]string, error) {
	var cfg ConfigWithLabels
	if err := json.Unmarshal(configJSON, &cfg); err != nil {
		return nil, fmt.Errorf("unmarshal labels config: %w", err)
	}

	if cfg.Config.Labels == nil {
		return make(map[string]string), nil
	}

	return cfg.Config.Labels, nil
}
