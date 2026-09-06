// Package imagestore provides OCI image configuration inspection utilities.
// This file implements an auditor for config.Healthcheck.StartInterval
// (the custom health probe interval applied during the container start period).

package imagestore

import (
	"encoding/json"
	"fmt"
	"time"
)

// healthcheckStartIntervalConfig represents the subset of Image Config JSON for StartInterval.
type healthcheckStartIntervalConfig struct {
	Config struct {
		Healthcheck *struct {
			StartInterval int64 `json:"StartInterval,omitempty"` // in nanoseconds
		} `json:"Healthcheck,omitempty"`
	} `json:"config"`
}

// ExtractHealthcheckStartInterval parses an OCI Image Config JSON blob and returns
// the configured StartInterval duration and whether it was explicitly configured.
func ExtractHealthcheckStartInterval(configJSON []byte) (time.Duration, bool, error) {
	var cfg healthcheckStartIntervalConfig
	if err := json.Unmarshal(configJSON, &cfg); err != nil {
		return 0, false, fmt.Errorf("parse image config for healthcheck start interval: %w", err)
	}

	if cfg.Config.Healthcheck == nil || cfg.Config.Healthcheck.StartInterval <= 0 {
		return 0, false, nil
	}

	return time.Duration(cfg.Config.Healthcheck.StartInterval), true, nil
}

// FormatHealthcheckStartInterval returns a human-readable summary of the start interval.
func FormatHealthcheckStartInterval(configJSON []byte) string {
	d, ok, err := ExtractHealthcheckStartInterval(configJSON)
	if err != nil {
		return fmt.Sprintf("error: %v", err)
	}
	if !ok {
		return "Healthcheck StartInterval: (not set)"
	}
	return fmt.Sprintf("Healthcheck StartInterval: %s", d)
}
