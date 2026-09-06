package imagestore

import (
	"encoding/json"
)

type ConfigWithRootFS struct {
	RootFS struct {
		Type    string   `json:"type"`
		DiffIDs []string `json:"diff_ids"`
	} `json:"rootfs"`
}

// ExtractRootFSDiffIDs extracts rootfs.diff_ids array from Image Config JSON.
func ExtractRootFSDiffIDs(configJSON []byte) []string {
	var cfg ConfigWithRootFS
	if err := json.Unmarshal(configJSON, &cfg); err == nil {
		return cfg.RootFS.DiffIDs
	}
	return nil
}
