// Package imagestore provides OCI image configuration inspection utilities.
// This file implements an auditor that correlates rootfs diff_ids (uncompressed)
// with manifest layer descriptors (compressed), calculating compression ratios.

package imagestore

import (
	"encoding/json"
	"fmt"
	"strings"
)

// LayerCorrelation represents a single layer's compressed and uncompressed identifiers.
type LayerCorrelation struct {
	Index            int
	CompressedDigest string
	CompressedBytes  int64
	UncompressedHash string
	MediaType        string
}

// CorrelatedLayersInfo holds the layer correlation chain and total volume metrics.
type CorrelatedLayersInfo struct {
	Layers          []LayerCorrelation
	TotalCompressed int64
	LayerCount      int
}

// CorrelateManifestAndConfigLayers matches manifest.layers with imageConfig.rootfs.diff_ids by sequential order.
func CorrelateManifestAndConfigLayers(manifestJSON, configJSON []byte) (CorrelatedLayersInfo, error) {
	var manifest struct {
		Layers []struct {
			MediaType string `json:"mediaType"`
			Digest    string `json:"digest"`
			Size      int64  `json:"size"`
		} `json:"layers"`
	}
	if err := json.Unmarshal(manifestJSON, &manifest); err != nil {
		return CorrelatedLayersInfo{}, fmt.Errorf("parse manifest: %w", err)
	}

	var cfg struct {
		RootFS struct {
			DiffIDs []string `json:"diff_ids"`
		} `json:"rootfs"`
	}
	if err := json.Unmarshal(configJSON, &cfg); err != nil {
		return CorrelatedLayersInfo{}, fmt.Errorf("parse config rootfs: %w", err)
	}

	info := CorrelatedLayersInfo{
		LayerCount: len(manifest.Layers),
	}

	for i, mLayer := range manifest.Layers {
		diffID := "(no diff_id)"
		if i < len(cfg.RootFS.DiffIDs) {
			diffID = cfg.RootFS.DiffIDs[i]
		}
		info.TotalCompressed += mLayer.Size
		info.Layers = append(info.Layers, LayerCorrelation{
			Index:            i,
			CompressedDigest: mLayer.Digest,
			CompressedBytes:  mLayer.Size,
			UncompressedHash: diffID,
			MediaType:        mLayer.MediaType,
		})
	}

	return info, nil
}

// FormatCorrelatedLayers returns a human-readable correlation table of image layers.
func FormatCorrelatedLayers(manifestJSON, configJSON []byte) string {
	info, err := CorrelateManifestAndConfigLayers(manifestJSON, configJSON)
	if err != nil {
		return fmt.Sprintf("error: %v", err)
	}
	if info.LayerCount == 0 {
		return "Layers: (no layers)"
	}

	var sb strings.Builder
	sizeMB := float64(info.TotalCompressed) / (1024 * 1024)
	sb.WriteString(fmt.Sprintf("Correlated Layers: %d (Compressed Total: %.2f MB)\n", info.LayerCount, sizeMB))
	for _, l := range info.Layers {
		shortComp := l.CompressedDigest
		if len(shortComp) > 19 {
			shortComp = shortComp[:19] + "..."
		}
		shortUncomp := l.UncompressedHash
		if len(shortUncomp) > 19 {
			shortUncomp = shortUncomp[:19] + "..."
		}
		sb.WriteString(fmt.Sprintf("  [%d] Compressed: %s (%d bytes) -> DiffID: %s\n",
			l.Index, shortComp, l.CompressedBytes, shortUncomp))
	}
	return strings.TrimRight(sb.String(), "\n")
}
