// Package imagestore provides OCI image configuration inspection utilities.
// This file implements an auditor for config.ExposedPorts definitions from OCI Image Config JSON.

package imagestore

import (
	"encoding/json"
	"fmt"
	"sort"
	"strconv"
	"strings"
)

// exposedPortsConfig represents the subset of Image Config JSON for exposed ports.
type exposedPortsConfig struct {
	Config struct {
		ExposedPorts map[string]struct{} `json:"ExposedPorts,omitempty"`
	} `json:"config"`
}

// ExposedPortEntry represents a single parsed exposed port and its protocol.
type ExposedPortEntry struct {
	Port     int
	Protocol string // "tcp", "udp", "sctp", etc.
}

// ExposedPortsSummary contains structured lists of exposed ports.
type ExposedPortsSummary struct {
	TotalPorts int
	Ports      []ExposedPortEntry
	TCPPorts   []int
	UDPPorts   []int
}

// ExtractExposedPorts parses an OCI Image Config JSON blob and returns
// sorted, categorized exposed network port definitions.
func ExtractExposedPorts(configJSON []byte) (ExposedPortsSummary, error) {
	var cfg exposedPortsConfig
	if err := json.Unmarshal(configJSON, &cfg); err != nil {
		return ExposedPortsSummary{}, fmt.Errorf("parse image config for exposed ports: %w", err)
	}

	if len(cfg.Config.ExposedPorts) == 0 {
		return ExposedPortsSummary{}, nil
	}

	var entries []ExposedPortEntry
	var tcpPorts, udpPorts []int

	for rawPort := range cfg.Config.ExposedPorts {
		parts := strings.SplitN(rawPort, "/", 2)
		portNum, err := strconv.Atoi(parts[0])
		if err != nil {
			continue
		}
		proto := "tcp"
		if len(parts) == 2 && parts[1] != "" {
			proto = strings.ToLower(parts[1])
		}

		entry := ExposedPortEntry{
			Port:     portNum,
			Protocol: proto,
		}
		entries = append(entries, entry)

		if proto == "udp" {
			udpPorts = append(udpPorts, portNum)
		} else if proto == "tcp" {
			tcpPorts = append(tcpPorts, portNum)
		}
	}

	// Sort entries by port number
	sort.Slice(entries, func(i, j int) bool {
		if entries[i].Port == entries[j].Port {
			return entries[i].Protocol < entries[j].Protocol
		}
		return entries[i].Port < entries[j].Port
	})
	sort.Ints(tcpPorts)
	sort.Ints(udpPorts)

	return ExposedPortsSummary{
		TotalPorts: len(entries),
		Ports:      entries,
		TCPPorts:   tcpPorts,
		UDPPorts:   udpPorts,
	}, nil
}

// FormatExposedPorts returns a human-readable summary of image exposed ports.
func FormatExposedPorts(configJSON []byte) string {
	summary, err := ExtractExposedPorts(configJSON)
	if err != nil {
		return fmt.Sprintf("error: %v", err)
	}
	if summary.TotalPorts == 0 {
		return "Exposed Ports: none"
	}

	var formatted []string
	for _, entry := range summary.Ports {
		formatted = append(formatted, fmt.Sprintf("%d/%s", entry.Port, entry.Protocol))
	}
	return fmt.Sprintf("Exposed Ports (%d): %s", summary.TotalPorts, strings.Join(formatted, ", "))
}
