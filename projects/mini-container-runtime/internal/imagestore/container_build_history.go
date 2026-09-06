// Package imagestore provides OCI image configuration inspection utilities.
// This file implements an auditor for image diff layer history entries
// from OCI Image Config JSON.

package imagestore

import (
	"encoding/json"
	"fmt"
	"strings"
)

// HistoryEntry represents a single layer build instruction from image history.
type HistoryEntry struct {
	CreatedBy  string `json:"created_by,omitempty"`
	EmptyLayer bool   `json:"empty_layer,omitempty"`
	Comment    string `json:"comment,omitempty"`
}

// historyConfig represents the subset of Image Config JSON for build history.
type historyConfig struct {
	History []HistoryEntry `json:"history,omitempty"`
}

// ExtractBuildHistory parses an OCI Image Config JSON blob and returns
// the list of layer build history instructions (Dockerfile commands).
func ExtractBuildHistory(configJSON []byte) ([]HistoryEntry, error) {
	var cfg historyConfig
	if err := json.Unmarshal(configJSON, &cfg); err != nil {
		return nil, fmt.Errorf("parse image config for build history: %w", err)
	}
	return cfg.History, nil
}

// FormatBuildHistory returns a human-readable summary of build history layers.
func FormatBuildHistory(configJSON []byte) string {
	entries, err := ExtractBuildHistory(configJSON)
	if err != nil {
		return fmt.Sprintf("error: %v", err)
	}
	if len(entries) == 0 {
		return "Build History: (none)"
	}

	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("Build History: %d layer(s)\n", len(entries)))
	for i, e := range entries {
		marker := "LAYER"
		if e.EmptyLayer {
			marker = "META "
		}
		cmd := e.CreatedBy
		if len(cmd) > 80 {
			cmd = cmd[:77] + "..."
		}
		sb.WriteString(fmt.Sprintf("  [%d] %s %s\n", i, marker, cmd))
	}
	return sb.String()
}
