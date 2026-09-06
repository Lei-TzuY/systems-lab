// Package imagestore provides OCI image configuration inspection utilities.
// This file reconstructs a best-effort Dockerfile from OCI Image Config
// history entries, extracting RUN/COPY/ADD/ENV/WORKDIR/EXPOSE/CMD/ENTRYPOINT directives.

package imagestore

import (
	"encoding/json"
	"fmt"
	"regexp"
	"strings"
)

// DockerfileInstruction represents a single reconstructed Dockerfile directive.
type DockerfileInstruction struct {
	Command    string // RUN, COPY, ADD, ENV, WORKDIR, EXPOSE, CMD, ENTRYPOINT, USER, VOLUME, STOPSIGNAL, LABEL, etc.
	Arguments  string
	EmptyLayer bool
}

var (
	runCmdRegex = regexp.MustCompile(`(?i)^/bin/sh -c (.+)$`)
	nopCmdRegex = regexp.MustCompile(`(?i)^(?:/bin/sh -c )?#\s*\(nop\)\s+(.+)$`)

	knownDirectives = map[string]struct{}{
		"ENV":         {},
		"WORKDIR":     {},
		"EXPOSE":      {},
		"CMD":         {},
		"ENTRYPOINT":  {},
		"USER":        {},
		"VOLUME":      {},
		"STOPSIGNAL":  {},
		"LABEL":       {},
		"HEALTHCHECK": {},
		"COPY":        {},
		"ADD":         {},
		"MAINTAINER":  {},
		"ARG":         {},
	}
)

// ReconstructDockerfile parses image config JSON and reconstructs Dockerfile instructions.
func ReconstructDockerfile(configJSON []byte) ([]DockerfileInstruction, error) {
	var cfg struct {
		History []struct {
			CreatedBy  string `json:"created_by,omitempty"`
			EmptyLayer bool   `json:"empty_layer,omitempty"`
		} `json:"history,omitempty"`
	}
	if err := json.Unmarshal(configJSON, &cfg); err != nil {
		return nil, fmt.Errorf("parse image config for Dockerfile reconstruction: %w", err)
	}

	var instructions []DockerfileInstruction
	for _, h := range cfg.History {
		cmd := strings.TrimSpace(h.CreatedBy)
		if cmd == "" {
			continue
		}

		inst := DockerfileInstruction{EmptyLayer: h.EmptyLayer}

		// 1. Match #(nop) directives (ENV, WORKDIR, EXPOSE, CMD, ENTRYPOINT, etc.)
		if m := nopCmdRegex.FindStringSubmatch(cmd); len(m) == 2 {
			directive := strings.TrimSpace(m[1])
			parts := strings.SplitN(directive, " ", 2)
			inst.Command = strings.ToUpper(parts[0])
			if len(parts) == 2 {
				inst.Arguments = strings.TrimSpace(parts[1])
			}
		} else if isDirectKnownDirective(cmd, &inst) {
			// Direct OCI instruction (e.g. ENTRYPOINT ["/app"], WORKDIR /app)
			// inst is populated by isDirectKnownDirective
		} else if m := runCmdRegex.FindStringSubmatch(cmd); len(m) == 2 {
			// Match /bin/sh -c <command> -> RUN <command>
			inst.Command = "RUN"
			inst.Arguments = strings.TrimSpace(m[1])
		} else {
			// Fallback raw command -> RUN <cmd>
			inst.Command = "RUN"
			inst.Arguments = cmd
		}

		instructions = append(instructions, inst)
	}

	return instructions, nil
}

func isDirectKnownDirective(cmd string, inst *DockerfileInstruction) bool {
	parts := strings.SplitN(cmd, " ", 2)
	candidate := strings.ToUpper(parts[0])
	if _, ok := knownDirectives[candidate]; ok {
		inst.Command = candidate
		if len(parts) == 2 {
			inst.Arguments = strings.TrimSpace(parts[1])
		}
		return true
	}
	return false
}

// FormatReconstructedDockerfile returns a formatted Dockerfile string.
func FormatReconstructedDockerfile(configJSON []byte) string {
	instructions, err := ReconstructDockerfile(configJSON)
	if err != nil {
		return fmt.Sprintf("# error: %v", err)
	}
	if len(instructions) == 0 {
		return "# (no history entries found)"
	}

	var sb strings.Builder
	sb.WriteString("# Reconstructed Dockerfile (best-effort)\n")
	for _, inst := range instructions {
		if inst.Arguments != "" {
			sb.WriteString(fmt.Sprintf("%s %s\n", inst.Command, inst.Arguments))
		} else {
			sb.WriteString(fmt.Sprintf("%s\n", inst.Command))
		}
	}
	return strings.TrimRight(sb.String(), "\n")
}
