// Package imagestore provides OCI image configuration inspection utilities.
// This file implements an inspector for config.Healthcheck.Test command definitions
// from OCI Image Config JSON.

package imagestore

import (
	"encoding/json"
	"fmt"
	"strings"
)

// HealthcheckType represents the type of healthcheck command.
type HealthcheckType string

const (
	HealthcheckNone      HealthcheckType = "NONE"
	HealthcheckCmd       HealthcheckType = "CMD"
	HealthcheckCmdShell  HealthcheckType = "CMD-SHELL"
	HealthcheckUndefined HealthcheckType = "UNDEFINED"
)

// healthcheckTestConfig represents the subset of Image Config JSON for healthcheck tests.
type healthcheckTestConfig struct {
	Config struct {
		Healthcheck *struct {
			Test []string `json:"Test,omitempty"`
		} `json:"Healthcheck,omitempty"`
	} `json:"config"`
}

// HealthcheckTestInfo contains the parsed test type and executable arguments.
type HealthcheckTestInfo struct {
	Type    HealthcheckType
	Command []string
}

// ExtractHealthcheckTest parses an OCI Image Config JSON and returns the
// healthcheck test command type and parameters.
func ExtractHealthcheckTest(configJSON []byte) (HealthcheckTestInfo, error) {
	var cfg healthcheckTestConfig
	if err := json.Unmarshal(configJSON, &cfg); err != nil {
		return HealthcheckTestInfo{Type: HealthcheckUndefined}, fmt.Errorf("parse image config for healthcheck test: %w", err)
	}

	if cfg.Config.Healthcheck == nil || len(cfg.Config.Healthcheck.Test) == 0 {
		return HealthcheckTestInfo{Type: HealthcheckUndefined}, nil
	}

	testSlice := cfg.Config.Healthcheck.Test
	first := strings.ToUpper(testSlice[0])

	switch first {
	case "NONE":
		return HealthcheckTestInfo{Type: HealthcheckNone}, nil
	case "CMD":
		return HealthcheckTestInfo{
			Type:    HealthcheckCmd,
			Command: testSlice[1:],
		}, nil
	case "CMD-SHELL":
		return HealthcheckTestInfo{
			Type:    HealthcheckCmdShell,
			Command: testSlice[1:],
		}, nil
	default:
		// Direct command without CMD/CMD-SHELL prefix
		return HealthcheckTestInfo{
			Type:    HealthcheckCmd,
			Command: testSlice,
		}, nil
	}
}

// FormatHealthcheckTest returns a human-readable summary of the healthcheck test.
func FormatHealthcheckTest(configJSON []byte) string {
	info, err := ExtractHealthcheckTest(configJSON)
	if err != nil {
		return fmt.Sprintf("error: %v", err)
	}
	switch info.Type {
	case HealthcheckNone:
		return "Healthcheck: NONE (disabled)"
	case HealthcheckUndefined:
		return "Healthcheck: (not configured)"
	case HealthcheckCmdShell:
		return fmt.Sprintf("Healthcheck: CMD-SHELL %s", strings.Join(info.Command, " "))
	case HealthcheckCmd:
		return fmt.Sprintf("Healthcheck: CMD [%s]", strings.Join(info.Command, ", "))
	default:
		return "Healthcheck: (unknown)"
	}
}
