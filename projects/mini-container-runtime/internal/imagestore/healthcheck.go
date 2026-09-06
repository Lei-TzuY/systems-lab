package imagestore

import (
	"encoding/json"
	"fmt"
)

type ImageHealthcheckConfig struct {
	Test     []string `json:"Test"`
	Interval int64    `json:"Interval"`
	Timeout  int64    `json:"Timeout"`
	Retries  int      `json:"Retries"`
}

type ConfigWithHealthcheck struct {
	Config struct {
		Healthcheck *ImageHealthcheckConfig `json:"Healthcheck"`
	} `json:"config"`
}

// ExtractHealthcheck extracts embedded Healthcheck struct from Image Config JSON.
func ExtractHealthcheck(configJSON []byte) (*ImageHealthcheckConfig, error) {
	var cfg ConfigWithHealthcheck
	if err := json.Unmarshal(configJSON, &cfg); err != nil {
		return nil, fmt.Errorf("unmarshal healthcheck config: %w", err)
	}

	return cfg.Config.Healthcheck, nil
}
