// Package imagestore provides OCI image configuration inspection utilities.
// This file implements an auditor for image author and maintainer metadata
// from OCI Image Config JSON.

package imagestore

import (
	"encoding/json"
	"fmt"
)

// authorConfig represents the subset of Image Config JSON for author metadata.
type authorConfig struct {
	Author string `json:"author,omitempty"`
	Config struct {
		Author string `json:"Author,omitempty"`
		Labels map[string]string `json:"Labels,omitempty"`
	} `json:"config"`
}

// AuthorInfo contains parsed author and maintainer metadata.
type AuthorInfo struct {
	Author     string
	Maintainer string
}

// ExtractAuthorInfo parses an OCI Image Config JSON blob and returns
// the declared author and maintainer (from Labels).
func ExtractAuthorInfo(configJSON []byte) (AuthorInfo, error) {
	var cfg authorConfig
	if err := json.Unmarshal(configJSON, &cfg); err != nil {
		return AuthorInfo{}, fmt.Errorf("parse image config for author: %w", err)
	}

	author := cfg.Author
	if author == "" {
		author = cfg.Config.Author
	}

	maintainer := ""
	if cfg.Config.Labels != nil {
		maintainer = cfg.Config.Labels["maintainer"]
	}

	return AuthorInfo{
		Author:     author,
		Maintainer: maintainer,
	}, nil
}

// FormatAuthorInfo returns a human-readable summary of image author metadata.
func FormatAuthorInfo(configJSON []byte) string {
	info, err := ExtractAuthorInfo(configJSON)
	if err != nil {
		return fmt.Sprintf("error: %v", err)
	}

	author := info.Author
	if author == "" {
		author = "(unknown)"
	}

	if info.Maintainer != "" && info.Maintainer != info.Author {
		return fmt.Sprintf("Author: %s, Maintainer: %s", author, info.Maintainer)
	}
	return fmt.Sprintf("Author: %s", author)
}
