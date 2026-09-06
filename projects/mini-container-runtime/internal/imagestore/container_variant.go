// Package imagestore provides OCI image configuration inspection utilities.
// This file implements an auditor for image architecture and cpu variant
// (architecture, variant) declarations from OCI Image Config JSON.

package imagestore

import (
	"encoding/json"
	"fmt"
	"strings"
)

// variantConfig represents the subset of Image Config JSON for architecture and variant.
type variantConfig struct {
	Architecture string `json:"architecture,omitempty"`
	Variant      string `json:"variant,omitempty"`
	Config       struct {
		Architecture string `json:"Architecture,omitempty"`
		Variant      string `json:"Variant,omitempty"`
	} `json:"config"`
}

// ArchitectureVariantInfo contains normalized architecture and microarchitecture variant.
type ArchitectureVariantInfo struct {
	Architecture string
	Variant      string
}

// ExtractArchitectureVariant parses an OCI Image Config JSON blob and returns
// the declared architecture and CPU microarchitecture variant.
func ExtractArchitectureVariant(configJSON []byte) (ArchitectureVariantInfo, error) {
	var cfg variantConfig
	if err := json.Unmarshal(configJSON, &cfg); err != nil {
		return ArchitectureVariantInfo{}, fmt.Errorf("parse image config for architecture/variant: %w", err)
	}

	arch := cfg.Architecture
	if arch == "" {
		arch = cfg.Config.Architecture
	}
	if arch == "" {
		arch = "amd64" // default
	}

	variant := cfg.Variant
	if variant == "" {
		variant = cfg.Config.Variant
	}

	return ArchitectureVariantInfo{
		Architecture: strings.ToLower(arch),
		Variant:      strings.ToLower(variant),
	}, nil
}

// FormatArchitectureVariant returns a human-readable summary of image architecture and variant.
func FormatArchitectureVariant(configJSON []byte) string {
	info, err := ExtractArchitectureVariant(configJSON)
	if err != nil {
		return fmt.Sprintf("error: %v", err)
	}
	if info.Variant == "" {
		return fmt.Sprintf("Arch: %s", info.Architecture)
	}
	return fmt.Sprintf("Arch: %s (variant: %s)", info.Architecture, info.Variant)
}
