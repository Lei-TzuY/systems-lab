// Package imagestore provides OCI image configuration inspection utilities.
// This file implements an auditor for the config.AttachStdin, config.AttachStdout,
// and config.AttachStderr stream binding flags from OCI Image Config JSON.

package imagestore

import (
	"encoding/json"
	"fmt"
)

// attachConfig represents the subset of OCI Image Config for attach flags.
type attachConfig struct {
	Config struct {
		AttachStdin  bool `json:"AttachStdin,omitempty"`
		AttachStdout bool `json:"AttachStdout,omitempty"`
		AttachStderr bool `json:"AttachStderr,omitempty"`
	} `json:"config"`
}

// AttachFlags contains the stdio stream attachment configuration of an image.
type AttachFlags struct {
	Stdin  bool
	Stdout bool
	Stderr bool
}

// ExtractAttachFlags parses an OCI Image Config JSON blob and returns the
// container's default stdio attachment configuration.
func ExtractAttachFlags(configJSON []byte) (AttachFlags, error) {
	var cfg attachConfig
	if err := json.Unmarshal(configJSON, &cfg); err != nil {
		return AttachFlags{}, fmt.Errorf("parse image config for attach flags: %w", err)
	}

	return AttachFlags{
		Stdin:  cfg.Config.AttachStdin,
		Stdout: cfg.Config.AttachStdout,
		Stderr: cfg.Config.AttachStderr,
	}, nil
}

// FormatAttachFlags returns a human-readable summary of image stdio attachment flags.
func FormatAttachFlags(configJSON []byte) string {
	flags, err := ExtractAttachFlags(configJSON)
	if err != nil {
		return fmt.Sprintf("error: %v", err)
	}
	return fmt.Sprintf("Attach: stdin=%t, stdout=%t, stderr=%t",
		flags.Stdin, flags.Stdout, flags.Stderr)
}
