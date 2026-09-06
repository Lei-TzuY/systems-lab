// Package imagestore provides OCI image configuration inspection utilities.
// This file implements an auditor for rootfs diff layer IDs from OCI Image Config JSON,
// providing layer count, total chain size, and per-layer digest listing.

package imagestore

import (
	"encoding/json"
	"fmt"
	"strings"
)

// rootfsConfig represents the subset of Image Config JSON for rootfs layers.
type rootfsConfig struct {
	RootFS struct {
		Type    string   `json:"type"`
		DiffIDs []string `json:"diff_ids"`
	} `json:"rootfs"`
}

// RootFSInfo contains parsed rootfs layer metadata.
type RootFSInfo struct {
	Type       string
	DiffIDs    []string
	LayerCount int
}

// ExtractRootFSLayers parses an OCI Image Config JSON blob and returns
// the rootfs type and layer diff IDs.
func ExtractRootFSLayers(configJSON []byte) (RootFSInfo, error) {
	var cfg rootfsConfig
	if err := json.Unmarshal(configJSON, &cfg); err != nil {
		return RootFSInfo{}, fmt.Errorf("parse image config for rootfs layers: %w", err)
	}

	return RootFSInfo{
		Type:       cfg.RootFS.Type,
		DiffIDs:    cfg.RootFS.DiffIDs,
		LayerCount: len(cfg.RootFS.DiffIDs),
	}, nil
}

// FormatRootFSLayers returns a human-readable summary of rootfs layers.
func FormatRootFSLayers(configJSON []byte) string {
	info, err := ExtractRootFSLayers(configJSON)
	if err != nil {
		return fmt.Sprintf("error: %v", err)
	}
	if info.LayerCount == 0 {
		return "RootFS: (no layers)"
	}

	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("RootFS Type: %s, Layers: %d\n", info.Type, info.LayerCount))
	for i, id := range info.DiffIDs {
		short := id
		if len(id) > 24 {
			short = id[:24] + "..."
		}
		sb.WriteString(fmt.Sprintf("  [%d] %s\n", i, short))
	}
	return sb.String()
}
