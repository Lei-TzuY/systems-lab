// internal/compose/compose.go
//
// Mini Compose — Multi-container Orchestration (`minictl compose up`)
// ────────────────────────────────────────────────────────────────────
// Parses a multi-container specification (JSON) and launches all services,
// setting up environment variables, ports, volumes, and overlay layers.

package compose

import (
	"encoding/json"
	"fmt"
	"os"

	"minicontainer/internal/container"
)

// ServiceSpec describes one container service in a compose file.
type ServiceSpec struct {
	Image       string            `json:"image"`       // RootFS path or image name
	Command     []string          `json:"command"`     // Command to execute
	Hostname    string            `json:"hostname"`    // Hostname
	WorkDir     string            `json:"workdir"`     // Working directory
	Overlay     bool              `json:"overlay"`     // Use OverlayFS
	ReadOnly    bool              `json:"read_only"`   // Read-only rootfs
	Ports       []string          `json:"ports"`       // Port mappings (e.g. "8080:80")
	Environment map[string]string `json:"environment"` // Environment variables
	Volumes     []string          `json:"volumes"`     // Volume mounts (e.g. "/host:/container")
}

// Config represents a full compose specification file.
type Config struct {
	Version  string                 `json:"version"`
	Services map[string]ServiceSpec `json:"services"`
}

// ParseConfigFile loads and decodes a compose JSON configuration file.
func ParseConfigFile(filePath string) (*Config, error) {
	data, err := os.ReadFile(filePath)
	if err != nil {
		return nil, fmt.Errorf("read compose file: %w", err)
	}

	var cfg Config
	if err := json.Unmarshal(data, &cfg); err != nil {
		return nil, fmt.Errorf("parse compose JSON: %w", err)
	}

	if len(cfg.Services) == 0 {
		return nil, fmt.Errorf("no services defined in compose file")
	}
	return &cfg, nil
}

// BuildContainerConfig converts a ServiceSpec into a runtime container.Config.
func (s *ServiceSpec) BuildContainerConfig(serviceName string) container.Config {
	hostname := s.Hostname
	if hostname == "" {
		hostname = serviceName
	}

	envs := make([]string, 0, len(s.Environment))
	for k, v := range s.Environment {
		envs = append(envs, k+"="+v)
	}

	return container.Config{
		RootFS:   s.Image,
		Command:  s.Command,
		Hostname: hostname,
		WorkDir:  s.WorkDir,
		Overlay:  s.Overlay,
		ReadOnly: s.ReadOnly,
		Env:      envs,
		UserNS:   true,
	}
}
