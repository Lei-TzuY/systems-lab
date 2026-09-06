// Package imagestore provides OCI image configuration inspection utilities.
// This file implements a multi-platform index / manifest list resolver
// that matches the target OS, Architecture, and Variant against available descriptors.

package imagestore

import (
	"encoding/json"
	"fmt"
	"strings"
)

// PlatformDescriptor represents platform architecture requirements for an image manifest.
type PlatformDescriptor struct {
	Architecture string   `json:"architecture"`
	OS           string   `json:"os"`
	OSVersion    string   `json:"os.version,omitempty"`
	OSFeatures   []string `json:"os.features,omitempty"`
	Variant      string   `json:"variant,omitempty"`
}

// ManifestListEntry represents an item in an OCI Index or Docker Manifest List.
type ManifestListEntry struct {
	MediaType string             `json:"mediaType"`
	Digest    string             `json:"digest"`
	Size      int64              `json:"size"`
	Platform  PlatformDescriptor `json:"platform"`
}

// manifestListConfig represents the manifests array in index.json or manifest list.
type manifestListConfig struct {
	SchemaVersion int                 `json:"schemaVersion"`
	MediaType     string              `json:"mediaType,omitempty"`
	Manifests     []ManifestListEntry `json:"manifests"`
}

// MatchPlatformManifest finds the best matching manifest descriptor from a manifest list.
func MatchPlatformManifest(indexJSON []byte, targetOS, targetArch, targetVariant string) (*ManifestListEntry, error) {
	var cfg manifestListConfig
	if err := json.Unmarshal(indexJSON, &cfg); err != nil {
		return nil, fmt.Errorf("parse manifest list: %w", err)
	}

	targetOS = strings.ToLower(targetOS)
	targetArch = strings.ToLower(targetArch)
	targetVariant = strings.ToLower(targetVariant)

	var fallbackArchMatch *ManifestListEntry

	for i := range cfg.Manifests {
		m := &cfg.Manifests[i]
		if strings.ToLower(m.Platform.OS) != targetOS {
			continue
		}
		if strings.ToLower(m.Platform.Architecture) != targetArch {
			continue
		}

		// If variant matches or no variant was requested
		mVar := strings.ToLower(m.Platform.Variant)
		if targetVariant == "" || mVar == targetVariant {
			return m, nil
		}

		if fallbackArchMatch == nil {
			fallbackArchMatch = m
		}
	}

	if fallbackArchMatch != nil {
		return fallbackArchMatch, nil
	}

	return nil, fmt.Errorf("no matching manifest found for platform %s/%s/%s", targetOS, targetArch, targetVariant)
}

// FormatPlatformManifests returns a human-readable list of all platforms in the index.
func FormatPlatformManifests(indexJSON []byte) string {
	var cfg manifestListConfig
	if err := json.Unmarshal(indexJSON, &cfg); err != nil {
		return fmt.Sprintf("error: %v", err)
	}
	if len(cfg.Manifests) == 0 {
		return "Manifests: (empty index)"
	}

	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("Manifest List: %d platforms available\n", len(cfg.Manifests)))
	for i, m := range cfg.Manifests {
		plat := fmt.Sprintf("%s/%s", m.Platform.OS, m.Platform.Architecture)
		if m.Platform.Variant != "" {
			plat += "/" + m.Platform.Variant
		}
		shortDigest := m.Digest
		if len(shortDigest) > 19 {
			shortDigest = shortDigest[:19] + "..."
		}
		sb.WriteString(fmt.Sprintf("  [%d] %-20s (Digest: %s)\n", i, plat, shortDigest))
	}
	return strings.TrimRight(sb.String(), "\n")
}
