// Package imagestore provides OCI image configuration inspection utilities.
// This file implements an auditor for config.Healthcheck timing intervals
// (Interval, Timeout, StartPeriod, Retries) from OCI Image Config JSON.

package imagestore

import (
	"encoding/json"
	"fmt"
	"time"
)

// healthcheckTimingConfig represents the subset of Image Config JSON for healthcheck timings.
type healthcheckTimingConfig struct {
	Config struct {
		Healthcheck *struct {
			Interval    int64 `json:"Interval,omitempty"`    // in nanoseconds
			Timeout     int64 `json:"Timeout,omitempty"`     // in nanoseconds
			StartPeriod int64 `json:"StartPeriod,omitempty"` // in nanoseconds
			Retries     int   `json:"Retries,omitempty"`
		} `json:"Healthcheck,omitempty"`
	} `json:"config"`
}

// HealthcheckTimingInfo contains the parsed timing intervals and retry threshold.
type HealthcheckTimingInfo struct {
	Configured  bool
	Interval    time.Duration
	Timeout     time.Duration
	StartPeriod time.Duration
	Retries     int
}

// ExtractHealthcheckTimings parses an OCI Image Config JSON blob and returns the
// container's healthcheck interval, timeout, start period, and retry count.
func ExtractHealthcheckTimings(configJSON []byte) (HealthcheckTimingInfo, error) {
	var cfg healthcheckTimingConfig
	if err := json.Unmarshal(configJSON, &cfg); err != nil {
		return HealthcheckTimingInfo{}, fmt.Errorf("parse image config for healthcheck timings: %w", err)
	}

	if cfg.Config.Healthcheck == nil {
		return HealthcheckTimingInfo{Configured: false}, nil
	}

	hc := cfg.Config.Healthcheck
	return HealthcheckTimingInfo{
		Configured:  true,
		Interval:    time.Duration(hc.Interval),
		Timeout:     time.Duration(hc.Timeout),
		StartPeriod: time.Duration(hc.StartPeriod),
		Retries:     hc.Retries,
	}, nil
}

// FormatHealthcheckTimings returns a human-readable summary of the healthcheck timing parameters.
func FormatHealthcheckTimings(configJSON []byte) string {
	info, err := ExtractHealthcheckTimings(configJSON)
	if err != nil {
		return fmt.Sprintf("error: %v", err)
	}
	if !info.Configured {
		return "Healthcheck Timings: (not configured)"
	}
	return fmt.Sprintf("Healthcheck Timings: interval=%s, timeout=%s, start_period=%s, retries=%d",
		info.Interval, info.Timeout, info.StartPeriod, info.Retries)
}
