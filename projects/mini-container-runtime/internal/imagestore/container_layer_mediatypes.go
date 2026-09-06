// Package imagestore provides OCI image configuration inspection utilities.
// This file implements an auditor for image manifest layer mediaTypes and compression formats.

package imagestore

import (
	"encoding/json"
	"fmt"
	"strings"
)

// manifestLayersConfig represents the layers list in OCI/Docker manifests.
type manifestLayersConfig struct {
	Layers []struct {
		MediaType string `json:"mediaType"`
		Digest    string `json:"digest"`
		Size      int64  `json:"size"`
	} `json:"layers"`
}

// LayerCompressionInfo contains parsed layer mediaTypes and compression breakdown.
type LayerCompressionInfo struct {
	TotalLayers  int
	TotalBytes   int64
	Compressions map[string]int // e.g. "gzip": 3, "zstd": 1, "uncompressed": 0
	MediaTypes   []string
}

// ExtractLayerMediaTypes parses an OCI / Docker Manifest JSON blob and returns
// layer count, total size, and detected compression formats.
func ExtractLayerMediaTypes(manifestJSON []byte) (LayerCompressionInfo, error) {
	var cfg manifestLayersConfig
	if err := json.Unmarshal(manifestJSON, &cfg); err != nil {
		return LayerCompressionInfo{}, fmt.Errorf("parse manifest layers: %w", err)
	}

	info := LayerCompressionInfo{
		TotalLayers:  len(cfg.Layers),
		Compressions: make(map[string]int),
	}

	for _, layer := range cfg.Layers {
		info.TotalBytes += layer.Size
		info.MediaTypes = append(info.MediaTypes, layer.MediaType)

		mt := strings.ToLower(layer.MediaType)
		if strings.Contains(mt, "gzip") || strings.HasSuffix(mt, ".tar+gzip") {
			info.Compressions["gzip"]++
		} else if strings.Contains(mt, "zstd") || strings.HasSuffix(mt, ".tar+zstd") {
			info.Compressions["zstd"]++
		} else if strings.Contains(mt, "squashfs") {
			info.Compressions["squashfs"]++
		} else if strings.Contains(mt, "tar") {
			info.Compressions["uncompressed"]++
		} else {
			info.Compressions["unknown"]++
		}
	}

	return info, nil
}

// FormatLayerMediaTypes returns a human-readable summary of image layer compression.
func FormatLayerMediaTypes(manifestJSON []byte) string {
	info, err := ExtractLayerMediaTypes(manifestJSON)
	if err != nil {
		return fmt.Sprintf("error: %v", err)
	}
	if info.TotalLayers == 0 {
		return "Layers: (no layers)"
	}

	var compParts []string
	for k, count := range info.Compressions {
		compParts = append(compParts, fmt.Sprintf("%s: %d", k, count))
	}

	sizeMB := float64(info.TotalBytes) / (1024 * 1024)
	return fmt.Sprintf("Layers: %d (%.2f MB), Formats: [%s]",
		info.TotalLayers, sizeMB, strings.Join(compParts, ", "))
}
