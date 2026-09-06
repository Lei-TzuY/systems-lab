// Package imagestore provides OCI image configuration inspection utilities.
// This file implements a label policy compliance checker that validates
// required OCI recommended labels and custom organizational label policies.

package imagestore

import (
	"encoding/json"
	"fmt"
	"strings"
)

// LabelPolicyResult contains the compliance evaluation of a single label.
type LabelPolicyResult struct {
	Label    string
	Required bool
	Present  bool
	Value    string
}

// LabelPolicyReport contains the full policy compliance report.
type LabelPolicyReport struct {
	Results     []LabelPolicyResult
	TotalLabels int
	Missing     int
	Score       int // 0-100
}

// DefaultRequiredLabels are the OCI-recommended image labels.
var DefaultRequiredLabels = []string{
	"org.opencontainers.image.title",
	"org.opencontainers.image.description",
	"org.opencontainers.image.version",
	"org.opencontainers.image.vendor",
	"org.opencontainers.image.url",
	"org.opencontainers.image.source",
	"org.opencontainers.image.licenses",
}

// AuditLabelPolicy checks image config labels against a required label policy.
// If requiredLabels is nil, DefaultRequiredLabels is used.
func AuditLabelPolicy(configJSON []byte, requiredLabels []string) (LabelPolicyReport, error) {
	var cfg struct {
		Config struct {
			Labels map[string]string `json:"Labels,omitempty"`
		} `json:"config"`
	}
	if err := json.Unmarshal(configJSON, &cfg); err != nil {
		return LabelPolicyReport{}, fmt.Errorf("parse config for label policy audit: %w", err)
	}

	if requiredLabels == nil {
		requiredLabels = DefaultRequiredLabels
	}

	labels := cfg.Config.Labels
	if labels == nil {
		labels = make(map[string]string)
	}

	report := LabelPolicyReport{
		TotalLabels: len(labels),
	}

	found := 0
	for _, req := range requiredLabels {
		val, present := labels[req]
		result := LabelPolicyResult{
			Label:    req,
			Required: true,
			Present:  present,
			Value:    val,
		}
		if present && strings.TrimSpace(val) != "" {
			found++
		} else {
			report.Missing++
		}
		report.Results = append(report.Results, result)
	}

	if len(requiredLabels) > 0 {
		report.Score = (found * 100) / len(requiredLabels)
	} else {
		report.Score = 100
	}

	return report, nil
}

// FormatLabelPolicyReport returns a human-readable label compliance summary.
func FormatLabelPolicyReport(configJSON []byte) string {
	report, err := AuditLabelPolicy(configJSON, nil)
	if err != nil {
		return fmt.Sprintf("error: %v", err)
	}

	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("Label Policy Score: %d/100 (missing %d/%d required)\n",
		report.Score, report.Missing, len(report.Results)))
	for _, r := range report.Results {
		status := "✓"
		if !r.Present {
			status = "✗"
		}
		sb.WriteString(fmt.Sprintf("  %s %s", status, r.Label))
		if r.Present {
			sb.WriteString(fmt.Sprintf(" = %q", r.Value))
		}
		sb.WriteString("\n")
	}
	return strings.TrimRight(sb.String(), "\n")
}
