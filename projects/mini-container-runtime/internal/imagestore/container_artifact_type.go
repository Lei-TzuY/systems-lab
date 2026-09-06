// Package imagestore provides OCI image configuration inspection utilities.
// This file implements an auditor for OCI 1.1 artifactType and subject descriptors
// (used for image signatures, SBOMs, and attestations).

package imagestore

import (
	"encoding/json"
	"fmt"
	"strings"
)

// artifactManifestConfig represents the OCI 1.1 artifact fields in manifest JSON.
type artifactManifestConfig struct {
	MediaType    string             `json:"mediaType,omitempty"`
	ArtifactType string             `json:"artifactType,omitempty"`
	Subject      *SubjectDescriptor `json:"subject,omitempty"`
}

// OCIArtifactInfo contains parsed artifact metadata.
type OCIArtifactInfo struct {
	ArtifactType string
	HasSubject   bool
	Subject      SubjectDescriptor
	IsArtifact   bool
}

// ExtractArtifactInfo parses an OCI Manifest JSON blob and extracts artifactType and subject.
func ExtractArtifactInfo(manifestJSON []byte) (OCIArtifactInfo, error) {
	var cfg artifactManifestConfig
	if err := json.Unmarshal(manifestJSON, &cfg); err != nil {
		return OCIArtifactInfo{}, fmt.Errorf("parse artifact manifest: %w", err)
	}

	artType := cfg.ArtifactType
	if artType == "" && strings.Contains(cfg.MediaType, "artifact") {
		artType = cfg.MediaType
	}

	info := OCIArtifactInfo{
		ArtifactType: artType,
		IsArtifact:   artType != "" || cfg.Subject != nil,
	}

	if cfg.Subject != nil && cfg.Subject.Digest != "" {
		info.HasSubject = true
		info.Subject = *cfg.Subject
	}

	return info, nil
}

// FormatArtifactInfo returns a human-readable summary of OCI artifact metadata.
func FormatArtifactInfo(manifestJSON []byte) string {
	info, err := ExtractArtifactInfo(manifestJSON)
	if err != nil {
		return fmt.Sprintf("error: %v", err)
	}
	if !info.IsArtifact {
		return "Artifact: standard container image"
	}

	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("Artifact Type: %s", info.ArtifactType))
	if info.HasSubject {
		shortDigest := info.Subject.Digest
		if len(shortDigest) > 19 {
			shortDigest = shortDigest[:19] + "..."
		}
		sb.WriteString(fmt.Sprintf(" (Subject: %s)", shortDigest))
	}
	return sb.String()
}
