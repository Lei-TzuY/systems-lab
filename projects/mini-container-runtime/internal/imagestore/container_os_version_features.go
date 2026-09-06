// Package imagestore provides OCI image configuration inspection utilities.
// This file implements an auditor for os.version and os.features fields from OCI Image Config JSON.

package imagestore

import (
	"encoding/json"
	"fmt"
	"strings"
)

// osVersionFeaturesConfig represents the subset of Image Config JSON for OS version and features.
type osVersionFeaturesConfig struct {
	OS         string   `json:"os,omitempty"`
	OSVersion  string   `json:"os.version,omitempty"`
	OSFeatures []string `json:"os.features,omitempty"`
	Config     struct {
		OSVersion  string   `json:"os.version,omitempty"`
		OSFeatures []string `json:"os.features,omitempty"`
	} `json:"config"`
}

// OSCompatibilityInfo contains parsed OS version and required kernel/platform features.
type OSCompatibilityInfo struct {
	OS         string
	OSVersion  string
	OSFeatures []string
}

// ExtractOSCompatibility parses an OCI Image Config JSON blob and returns
// the declared OS target, OS version string, and required platform features.
func ExtractOSCompatibility(configJSON []byte) (OSCompatibilityInfo, error) {
	var cfg osVersionFeaturesConfig
	if err := json.Unmarshal(configJSON, &cfg); err != nil {
		return OSCompatibilityInfo{}, fmt.Errorf("parse image config for os version/features: %w", err)
	}

	osTarget := cfg.OS
	if osTarget == "" {
		osTarget = "linux"
	}

	osVer := cfg.OSVersion
	if osVer == "" {
		osVer = cfg.Config.OSVersion
	}

	features := cfg.OSFeatures
	if len(features) == 0 {
		features = cfg.Config.OSFeatures
	}

	return OSCompatibilityInfo{
		OS:         osTarget,
		OSVersion:  osVer,
		OSFeatures: features,
	}, nil
}

// FormatOSCompatibility returns a human-readable summary of image OS version and required features.
func FormatOSCompatibility(configJSON []byte) string {
	info, err := ExtractOSCompatibility(configJSON)
	if err != nil {
		return fmt.Sprintf("error: %v", err)
	}

	ver := info.OSVersion
	if ver == "" {
		ver = "(any)"
	}

	var featStr string
	if len(info.OSFeatures) > 0 {
		featStr = fmt.Sprintf(", features=[%s]", strings.Join(info.OSFeatures, ", "))
	}

	return fmt.Sprintf("OS Target: %s, Version: %s%s", info.OS, ver, featStr)
}
