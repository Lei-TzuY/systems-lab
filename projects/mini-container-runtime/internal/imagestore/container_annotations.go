// Package imagestore provides OCI image configuration inspection utilities.
// This file implements an auditor for image manifest and descriptor annotations (OCI Spec).

package imagestore

import (
	"encoding/json"
	"fmt"
	"strings"
)

// manifestAnnotationsConfig represents the annotations dictionary in OCI manifests.
type manifestAnnotationsConfig struct {
	Annotations map[string]string `json:"annotations,omitempty"`
}

// StandardOCIAnnotations contains extracted well-known standard OCI annotation values.
type StandardOCIAnnotations struct {
	Title         string
	Description   string
	Version       string
	Revision      string
	Vendor        string
	Licenses      string
	Documentation string
	All           map[string]string
}

// ExtractManifestAnnotations parses an OCI Image Manifest JSON blob and extracts
// standard OCI annotation keys (org.opencontainers.image.*).
func ExtractManifestAnnotations(manifestJSON []byte) (StandardOCIAnnotations, error) {
	var cfg manifestAnnotationsConfig
	if err := json.Unmarshal(manifestJSON, &cfg); err != nil {
		return StandardOCIAnnotations{}, fmt.Errorf("parse manifest annotations: %w", err)
	}

	all := cfg.Annotations
	if all == nil {
		all = make(map[string]string)
	}

	return StandardOCIAnnotations{
		Title:         all["org.opencontainers.image.title"],
		Description:   all["org.opencontainers.image.description"],
		Version:       all["org.opencontainers.image.version"],
		Revision:      all["org.opencontainers.image.revision"],
		Vendor:        all["org.opencontainers.image.vendor"],
		Licenses:      all["org.opencontainers.image.licenses"],
		Documentation: all["org.opencontainers.image.documentation"],
		All:           all,
	}, nil
}

// FormatManifestAnnotations returns a human-readable summary of OCI annotations.
func FormatManifestAnnotations(manifestJSON []byte) string {
	ann, err := ExtractManifestAnnotations(manifestJSON)
	if err != nil {
		return fmt.Sprintf("error: %v", err)
	}
	if len(ann.All) == 0 {
		return "Annotations: (none)"
	}

	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("Annotations: %d entries\n", len(ann.All)))
	if ann.Title != "" {
		sb.WriteString(fmt.Sprintf("  Title: %s\n", ann.Title))
	}
	if ann.Version != "" {
		sb.WriteString(fmt.Sprintf("  Version: %s\n", ann.Version))
	}
	if ann.Vendor != "" {
		sb.WriteString(fmt.Sprintf("  Vendor: %s\n", ann.Vendor))
	}
	if ann.Licenses != "" {
		sb.WriteString(fmt.Sprintf("  Licenses: %s\n", ann.Licenses))
	}
	return strings.TrimRight(sb.String(), "\n")
}
