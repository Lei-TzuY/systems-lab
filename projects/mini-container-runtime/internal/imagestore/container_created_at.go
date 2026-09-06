// Package imagestore provides OCI image configuration inspection utilities.
// This file implements an auditor for image creation timestamp (created) from OCI Image Config JSON.

package imagestore

import (
	"encoding/json"
	"fmt"
	"time"
)

// createdAtConfig represents the subset of Image Config JSON for created timestamp.
type createdAtConfig struct {
	Created string `json:"created,omitempty"`
	Config  struct {
		Created string `json:"Created,omitempty"`
	} `json:"config"`
}

// CreatedAtInfo contains parsed creation timestamp and computed relative age.
type CreatedAtInfo struct {
	Timestamp time.Time
	HasTime   bool
}

// ExtractCreatedAt parses an OCI Image Config JSON blob and returns the creation time.
func ExtractCreatedAt(configJSON []byte) (CreatedAtInfo, error) {
	var cfg createdAtConfig
	if err := json.Unmarshal(configJSON, &cfg); err != nil {
		return CreatedAtInfo{}, fmt.Errorf("parse image config for created timestamp: %w", err)
	}

	rawTime := cfg.Created
	if rawTime == "" {
		rawTime = cfg.Config.Created
	}
	if rawTime == "" {
		return CreatedAtInfo{HasTime: false}, nil
	}

	// Try RFC3339Nano then RFC3339
	if t, err := time.Parse(time.RFC3339Nano, rawTime); err == nil {
		return CreatedAtInfo{Timestamp: t, HasTime: true}, nil
	}
	if t, err := time.Parse(time.RFC3339, rawTime); err == nil {
		return CreatedAtInfo{Timestamp: t, HasTime: true}, nil
	}

	return CreatedAtInfo{HasTime: false}, fmt.Errorf("unable to parse created timestamp %q", rawTime)
}

// FormatRelativeAge computes a human-readable duration relative to the provided base time.
func FormatRelativeAge(created, now time.Time) string {
	diff := now.Sub(created)
	if diff < 0 {
		return "in the future"
	}

	seconds := int(diff.Seconds())
	if seconds < 60 {
		return fmt.Sprintf("%d seconds ago", seconds)
	}
	minutes := int(diff.Minutes())
	if minutes < 60 {
		return fmt.Sprintf("%d minutes ago", minutes)
	}
	hours := int(diff.Hours())
	if hours < 24 {
		return fmt.Sprintf("%d hours ago", hours)
	}
	days := hours / 24
	if days < 30 {
		return fmt.Sprintf("%d days ago", days)
	}
	months := days / 30
	if months < 12 {
		return fmt.Sprintf("%d months ago", months)
	}
	years := days / 365
	return fmt.Sprintf("%d years ago", years)
}

// FormatCreatedAt returns a human-readable summary of image creation timestamp and age.
func FormatCreatedAt(configJSON []byte, now time.Time) string {
	info, err := ExtractCreatedAt(configJSON)
	if err != nil {
		return fmt.Sprintf("error: %v", err)
	}
	if !info.HasTime {
		return "Created: (unknown)"
	}
	return fmt.Sprintf("Created: %s (%s)",
		info.Timestamp.Format(time.RFC3339), FormatRelativeAge(info.Timestamp, now))
}
