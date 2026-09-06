// Package imagestore provides OCI image configuration inspection utilities.
// This file implements an auditor for OCI image history entries that distinguishes
// between metadata-only layers (empty_layer: true) and filesystem-modifying layers.

package imagestore

import (
	"encoding/json"
	"fmt"
)

// historyEmptyConfig represents the subset of Image Config JSON for history inspection.
type historyEmptyConfig struct {
	History []struct {
		Created    string `json:"created,omitempty"`
		CreatedBy  string `json:"created_by,omitempty"`
		EmptyLayer bool   `json:"empty_layer,omitempty"`
		Comment    string `json:"comment,omitempty"`
	} `json:"history"`
}

// LayerTypeSummary contains aggregated layer classification metrics.
type LayerTypeSummary struct {
	TotalLayers int
	EmptyLayers int
	DataLayers  int
}

// InspectEmptyLayers parses an OCI Image Config JSON blob and categorizes
// history entries into empty metadata-only layers and filesystem-altering data layers.
func InspectEmptyLayers(configJSON []byte) (LayerTypeSummary, error) {
	var cfg historyEmptyConfig
	if err := json.Unmarshal(configJSON, &cfg); err != nil {
		return LayerTypeSummary{}, fmt.Errorf("parse image config for empty layers: %w", err)
	}

	summary := LayerTypeSummary{
		TotalLayers: len(cfg.History),
	}

	for _, h := range cfg.History {
		if h.EmptyLayer {
			summary.EmptyLayers++
		} else {
			summary.DataLayers++
		}
	}

	return summary, nil
}

// FormatEmptyLayerSummary returns a human-readable summary of empty vs data layers.
func FormatEmptyLayerSummary(configJSON []byte) string {
	summary, err := InspectEmptyLayers(configJSON)
	if err != nil {
		return fmt.Sprintf("error: %v", err)
	}
	return fmt.Sprintf("Layers: %d total (%d data, %d metadata-only)",
		summary.TotalLayers, summary.DataLayers, summary.EmptyLayers)
}
