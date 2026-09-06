// Package imagestore provides OCI image configuration inspection utilities.
// This file implements an auditor for reproducible build compliance
// (deterministic timestamps, sorted environment variables, reproducible metadata).

package imagestore

import (
	"encoding/json"
	"fmt"
	"sort"
	"strings"
	"time"
)

// ReproducibilityReport contains the evaluation results of reproducible build checks.
type ReproducibilityReport struct {
	IsZeroTimestamp   bool
	IsDeterministic   bool
	SortedEnv         bool
	UnsortedEnvKeys   []string
	CreatedTimestamp  string
	ReproducibleScore int // 0 to 100
}

// AuditReproducibleBuild checks image config for deterministic build properties.
func AuditReproducibleBuild(configJSON []byte) (ReproducibilityReport, error) {
	var cfg struct {
		Created string `json:"created,omitempty"`
		Config  struct {
			Env     []string `json:"Env,omitempty"`
			Created string   `json:"Created,omitempty"`
		} `json:"config"`
	}
	if err := json.Unmarshal(configJSON, &cfg); err != nil {
		return ReproducibilityReport{}, fmt.Errorf("parse image config for reproducibility audit: %w", err)
	}

	report := ReproducibilityReport{
		SortedEnv: true,
	}

	created := cfg.Created
	if created == "" {
		created = cfg.Config.Created
	}
	report.CreatedTimestamp = created

	if t, err := time.Parse(time.RFC3339, created); err == nil {
		if t.Unix() == 0 || t.Year() == 1970 {
			report.IsZeroTimestamp = true
			report.IsDeterministic = true
		}
	} else if created == "" {
		report.IsZeroTimestamp = true
		report.IsDeterministic = true
	}

	// Check if Env vars are sorted alphabetically
	envs := cfg.Config.Env
	if len(envs) > 1 {
		sortedEnvs := make([]string, len(envs))
		copy(sortedEnvs, envs)
		sort.Strings(sortedEnvs)

		for i := range envs {
			if envs[i] != sortedEnvs[i] {
				report.SortedEnv = false
				report.UnsortedEnvKeys = append(report.UnsortedEnvKeys, envs[i])
			}
		}
	}

	score := 0
	if report.IsDeterministic {
		score += 50
	}
	if report.SortedEnv {
		score += 50
	}
	report.ReproducibleScore = score

	return report, nil
}

// FormatReproducibilityReport returns a human-readable summary of reproducibility checks.
func FormatReproducibilityReport(configJSON []byte) string {
	report, err := AuditReproducibleBuild(configJSON)
	if err != nil {
		return fmt.Sprintf("error: %v", err)
	}

	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("Reproducible Build Score: %d/100\n", report.ReproducibleScore))
	sb.WriteString(fmt.Sprintf("  Deterministic Timestamp: %t\n", report.IsDeterministic))
	sb.WriteString(fmt.Sprintf("  Sorted Environment: %t", report.SortedEnv))
	return sb.String()
}
