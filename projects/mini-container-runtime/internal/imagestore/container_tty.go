// Package imagestore provides OCI image configuration inspection utilities.
// This file implements an auditor for the config.Tty, config.OpenStdin, and
// config.StdinOnce terminal and input allocation settings from OCI Image Config JSON.

package imagestore

import (
	"encoding/json"
	"fmt"
)

// ttyConfig represents the subset of OCI Image Config for TTY and stdin settings.
type ttyConfig struct {
	Config struct {
		Tty       bool `json:"Tty,omitempty"`
		OpenStdin bool `json:"OpenStdin,omitempty"`
		StdinOnce bool `json:"StdinOnce,omitempty"`
	} `json:"config"`
}

// TTYFlags contains interactive terminal and stdin configuration.
type TTYFlags struct {
	Tty       bool
	OpenStdin bool
	StdinOnce bool
}

// ExtractTTYFlags parses an OCI Image Config JSON blob and returns the
// image's default TTY and stdin allocation settings.
func ExtractTTYFlags(configJSON []byte) (TTYFlags, error) {
	var cfg ttyConfig
	if err := json.Unmarshal(configJSON, &cfg); err != nil {
		return TTYFlags{}, fmt.Errorf("parse image config for tty flags: %w", err)
	}

	return TTYFlags{
		Tty:       cfg.Config.Tty,
		OpenStdin: cfg.Config.OpenStdin,
		StdinOnce: cfg.Config.StdinOnce,
	}, nil
}

// FormatTTYFlags returns a human-readable summary of image TTY/stdin settings.
func FormatTTYFlags(configJSON []byte) string {
	flags, err := ExtractTTYFlags(configJSON)
	if err != nil {
		return fmt.Sprintf("error: %v", err)
	}
	return fmt.Sprintf("TTY/Stdin: tty=%t, open_stdin=%t, stdin_once=%t",
		flags.Tty, flags.OpenStdin, flags.StdinOnce)
}
