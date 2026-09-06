// Package imagestore provides OCI image configuration inspection utilities.
// This file implements an auditor for the config.NetworkDisabled and config.MacAddress
// network isolation settings from OCI Image Config JSON.

package imagestore

import (
	"encoding/json"
	"fmt"
)

// networkConfig represents the subset of OCI Image Config for network settings.
type networkConfig struct {
	Config struct {
		NetworkDisabled bool   `json:"NetworkDisabled,omitempty"`
		MacAddress      string `json:"MacAddress,omitempty"`
	} `json:"config"`
}

// NetworkIsolationSettings contains network configuration metadata from an image.
type NetworkIsolationSettings struct {
	NetworkDisabled bool
	MacAddress      string
}

// ExtractNetworkSettings parses an OCI Image Config JSON blob and returns the
// image's declared network isolation and MAC address parameters.
func ExtractNetworkSettings(configJSON []byte) (NetworkIsolationSettings, error) {
	var cfg networkConfig
	if err := json.Unmarshal(configJSON, &cfg); err != nil {
		return NetworkIsolationSettings{}, fmt.Errorf("parse image config for network settings: %w", err)
	}

	return NetworkIsolationSettings{
		NetworkDisabled: cfg.Config.NetworkDisabled,
		MacAddress:      cfg.Config.MacAddress,
	}, nil
}

// FormatNetworkSettings returns a human-readable summary of image network isolation settings.
func FormatNetworkSettings(configJSON []byte) string {
	settings, err := ExtractNetworkSettings(configJSON)
	if err != nil {
		return fmt.Sprintf("error: %v", err)
	}
	mac := settings.MacAddress
	if mac == "" {
		mac = "(dynamic)"
	}
	return fmt.Sprintf("Network: disabled=%t, mac=%s", settings.NetworkDisabled, mac)
}
