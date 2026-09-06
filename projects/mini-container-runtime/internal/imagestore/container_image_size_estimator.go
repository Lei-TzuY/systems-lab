// Package imagestore provides OCI image configuration inspection utilities.
// This file implements an image size estimator calculating compressed download volume,
// estimated unpacked disk footprint, and layer breakdown.

package imagestore

import (
	"encoding/json"
	"fmt"
	"strings"
)

// ImageSizeEstimate contains download volume and estimated disk usage calculations.
type ImageSizeEstimate struct {
	ConfigSize       int64
	LayersCompressed int64
	TotalDownload    int64
	EstimatedDisk    int64
	LayerCount       int
}

// EstimateImageSizes parses manifest JSON and optional config JSON to compute size projections.
func EstimateImageSizes(manifestJSON []byte, configJSON []byte) (ImageSizeEstimate, error) {
	var manifest struct {
		Config struct {
			Size int64 `json:"size"`
		} `json:"config"`
		Layers []struct {
			Size int64 `json:"size"`
		} `json:"layers"`
	}
	if err := json.Unmarshal(manifestJSON, &manifest); err != nil {
		return ImageSizeEstimate{}, fmt.Errorf("parse manifest for size estimate: %w", err)
	}

	est := ImageSizeEstimate{
		ConfigSize: manifest.Config.Size,
		LayerCount: len(manifest.Layers),
	}

	for _, l := range manifest.Layers {
		est.LayersCompressed += l.Size
	}
	est.TotalDownload = est.ConfigSize + est.LayersCompressed

	// Estimated unpacked disk footprint (~2.5x compressed tar.gz/zstd size heuristic + config)
	est.EstimatedDisk = int64(float64(est.LayersCompressed)*2.5) + est.ConfigSize

	return est, nil
}

// FormatImageSizeEstimate returns a human-readable summary of image size projections.
func FormatImageSizeEstimate(manifestJSON []byte) string {
	est, err := EstimateImageSizes(manifestJSON, nil)
	if err != nil {
		return fmt.Sprintf("error: %v", err)
	}

	downloadMB := float64(est.TotalDownload) / (1024 * 1024)
	diskMB := float64(est.EstimatedDisk) / (1024 * 1024)

	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("Download Size: %.2f MB (%d bytes in %d layers)\n",
		downloadMB, est.TotalDownload, est.LayerCount))
	sb.WriteString(fmt.Sprintf("Estimated Unpacked Disk: ~%.2f MB", diskMB))
	return sb.String()
}
