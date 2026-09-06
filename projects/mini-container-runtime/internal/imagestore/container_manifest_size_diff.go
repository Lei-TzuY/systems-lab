// Package imagestore provides OCI image configuration inspection utilities.
// This file implements layer-by-layer diff and size growth comparison
// between two OCI image manifests (e.g., base version vs upgraded version).

package imagestore

import (
	"encoding/json"
	"fmt"
	"strings"
)

// LayerDescriptor represents a single layer in an OCI manifest.
type LayerDescriptor struct {
	MediaType string `json:"mediaType"`
	Digest    string `json:"digest"`
	Size      int64  `json:"size"`
}

// ManifestLayerDiff contains comparison metrics between two image manifests.
type ManifestLayerDiff struct {
	BaseLayersCount    int
	TargetLayersCount  int
	SharedLayersCount  int
	AddedLayersCount   int
	DeletedLayersCount int
	BaseTotalBytes     int64
	TargetTotalBytes   int64
	NetDeltaBytes      int64 // TargetTotalBytes - BaseTotalBytes
	AddedBytes         int64
	DeletedBytes       int64
	SharedBytes        int64
	ReuseRatioPercent  float64
}

// DiffImageManifestLayers computes the difference in layers and size between base and target manifests.
func DiffImageManifestLayers(baseManifestJSON, targetManifestJSON []byte) (ManifestLayerDiff, error) {
	var baseManifest, targetManifest struct {
		Layers []LayerDescriptor `json:"layers"`
	}

	if err := json.Unmarshal(baseManifestJSON, &baseManifest); err != nil {
		return ManifestLayerDiff{}, fmt.Errorf("parse base manifest: %w", err)
	}
	if err := json.Unmarshal(targetManifestJSON, &targetManifest); err != nil {
		return ManifestLayerDiff{}, fmt.Errorf("parse target manifest: %w", err)
	}

	// Validate layer sizes
	baseDigestSet := make(map[string]struct{})
	var baseTotal int64
	for i, l := range baseManifest.Layers {
		if l.Size < 0 {
			return ManifestLayerDiff{}, fmt.Errorf("base manifest layer %d has negative size %d", i, l.Size)
		}
		if l.Digest != "" {
			baseDigestSet[l.Digest] = struct{}{}
		}
		baseTotal += l.Size
	}

	targetDigestSet := make(map[string]struct{})
	var targetTotal int64
	for i, l := range targetManifest.Layers {
		if l.Size < 0 {
			return ManifestLayerDiff{}, fmt.Errorf("target manifest layer %d has negative size %d", i, l.Size)
		}
		if l.Digest != "" {
			targetDigestSet[l.Digest] = struct{}{}
		}
		targetTotal += l.Size
	}

	diff := ManifestLayerDiff{
		BaseLayersCount:   len(baseManifest.Layers),
		TargetLayersCount: len(targetManifest.Layers),
		BaseTotalBytes:    baseTotal,
		TargetTotalBytes:  targetTotal,
		NetDeltaBytes:     targetTotal - baseTotal,
	}

	// Process target layers in sequential order
	for _, l := range targetManifest.Layers {
		if _, exists := baseDigestSet[l.Digest]; exists && l.Digest != "" {
			diff.SharedLayersCount++
			diff.SharedBytes += l.Size
		} else {
			diff.AddedLayersCount++
			diff.AddedBytes += l.Size
		}
	}

	// Process base layers in sequential order
	for _, l := range baseManifest.Layers {
		if _, exists := targetDigestSet[l.Digest]; !exists || l.Digest == "" {
			diff.DeletedLayersCount++
			diff.DeletedBytes += l.Size
		}
	}

	if targetTotal > 0 {
		diff.ReuseRatioPercent = (float64(diff.SharedBytes) / float64(targetTotal)) * 100.0
		if diff.ReuseRatioPercent > 100.0 {
			diff.ReuseRatioPercent = 100.0
		}
	}

	return diff, nil
}

// FormatManifestLayerDiff returns a human-readable comparison summary.
func FormatManifestLayerDiff(baseManifestJSON, targetManifestJSON []byte) string {
	diff, err := DiffImageManifestLayers(baseManifestJSON, targetManifestJSON)
	if err != nil {
		return fmt.Sprintf("error: %v", err)
	}

	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("Manifest Layer Diff Summary:\n"))
	sb.WriteString(fmt.Sprintf("  Base: %d layers (%.2f MB)\n", diff.BaseLayersCount, float64(diff.BaseTotalBytes)/(1024*1024)))
	sb.WriteString(fmt.Sprintf("  Target: %d layers (%.2f MB)\n", diff.TargetLayersCount, float64(diff.TargetTotalBytes)/(1024*1024)))
	sb.WriteString(fmt.Sprintf("  Shared/Reused: %d layers (%.2f MB, %.1f%% reuse)\n",
		diff.SharedLayersCount, float64(diff.SharedBytes)/(1024*1024), diff.ReuseRatioPercent))
	sb.WriteString(fmt.Sprintf("  Added: +%d layers (+%.2f MB)\n", diff.AddedLayersCount, float64(diff.AddedBytes)/(1024*1024)))
	sb.WriteString(fmt.Sprintf("  Deleted: -%d layers (-%.2f MB)\n", diff.DeletedLayersCount, float64(diff.DeletedBytes)/(1024*1024)))
	sb.WriteString(fmt.Sprintf("  Net Growth: %+d bytes", diff.NetDeltaBytes))
	return sb.String()
}
