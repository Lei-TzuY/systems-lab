// Package imagestore provides OCI image configuration inspection utilities.
// This file implements an auditor for manifest schemaVersion and mediaType declarations.

package imagestore

import (
	"encoding/json"
	"fmt"
	"strings"
)

// manifestSchemaConfig represents the top-level schema fields of an OCI / Docker manifest.
type manifestSchemaConfig struct {
	SchemaVersion int    `json:"schemaVersion"`
	MediaType     string `json:"mediaType,omitempty"`
}

// ManifestSchemaInfo contains parsed manifest specification metadata.
type ManifestSchemaInfo struct {
	SchemaVersion int
	MediaType     string
	Format        string // "OCI", "Docker V2", "Docker V1", "Unknown"
}

// ExtractManifestSchema parses an OCI / Docker manifest JSON blob and returns
// the schemaVersion, mediaType, and detected specification format.
func ExtractManifestSchema(manifestJSON []byte) (ManifestSchemaInfo, error) {
	var cfg manifestSchemaConfig
	if err := json.Unmarshal(manifestJSON, &cfg); err != nil {
		return ManifestSchemaInfo{}, fmt.Errorf("parse manifest schema: %w", err)
	}

	if cfg.SchemaVersion <= 0 {
		cfg.SchemaVersion = 2
	}

	format := "Unknown"
	mt := cfg.MediaType
	if strings.Contains(mt, "oci") {
		format = "OCI v1"
	} else if strings.Contains(mt, "docker.distribution.manifest.v2") {
		format = "Docker Manifest v2"
	} else if strings.Contains(mt, "docker.distribution.manifest.list") {
		format = "Docker Manifest List v2"
	} else if cfg.SchemaVersion == 2 {
		format = "OCI / Docker v2"
	} else if cfg.SchemaVersion == 1 {
		format = "Docker Manifest v1 (Legacy)"
	}

	return ManifestSchemaInfo{
		SchemaVersion: cfg.SchemaVersion,
		MediaType:     cfg.MediaType,
		Format:        format,
	}, nil
}

// FormatManifestSchema returns a human-readable summary of the manifest schema.
func FormatManifestSchema(manifestJSON []byte) string {
	info, err := ExtractManifestSchema(manifestJSON)
	if err != nil {
		return fmt.Sprintf("error: %v", err)
	}
	if info.MediaType == "" {
		return fmt.Sprintf("SchemaVersion: %d (Format: %s)", info.SchemaVersion, info.Format)
	}
	return fmt.Sprintf("SchemaVersion: %d, MediaType: %s (Format: %s)",
		info.SchemaVersion, info.MediaType, info.Format)
}
