// Package logs provides container log processing utilities.
// This file implements a log template miner (inspired by the Drain algorithm)
// that extracts common parameterized templates from unstructured log streams.

package logs

import (
	"fmt"
	"regexp"
	"sort"
	"strings"
)

// LogTemplateMiner extracts recurring log templates by parameterizing variable tokens.
type LogTemplateMiner struct {
	variableRegexes []*regexp.Regexp
}

// NewLogTemplateMiner creates a LogTemplateMiner with default variable token extractors.
func NewLogTemplateMiner() *LogTemplateMiner {
	return &LogTemplateMiner{
		variableRegexes: []*regexp.Regexp{
			// UUIDs
			regexp.MustCompile(`\b[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}\b`),
			// Hex / Hashes (e.g., sha256 or git hashes)
			regexp.MustCompile(`\b[0-9a-fA-F]{16,64}\b`),
			// IPv4 addresses
			regexp.MustCompile(`\b(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\b`),
			// Numbers (integers, floats, port numbers)
			regexp.MustCompile(`\b\d+(?:\.\d+)?\b`),
			// ISO Timestamps in message
			regexp.MustCompile(`\b\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})?\b`),
		},
	}
}

// ExtractTemplate masks variables in a log line to produce a generalized template string.
func (m *LogTemplateMiner) ExtractTemplate(line string) string {
	res := line
	for _, re := range m.variableRegexes {
		res = re.ReplaceAllString(res, "<*>")
	}

	// Collapse multiple consecutive <*> into a single <*>
	collapseRegex := regexp.MustCompile(`(<\*>\s*)+`)
	res = collapseRegex.ReplaceAllString(res, "<*> ")

	return strings.TrimSpace(res)
}

// TemplateCluster represents a group of log lines sharing the same extracted template.
type TemplateCluster struct {
	Template string
	Count    int
	Examples []string
}

// MineClusters groups a slice of log lines into template clusters, sorted by frequency (descending).
func (m *LogTemplateMiner) MineClusters(lines []string) []TemplateCluster {
	clusters := make(map[string]*TemplateCluster)

	for _, line := range lines {
		tmpl := m.ExtractTemplate(line)
		if c, exists := clusters[tmpl]; exists {
			c.Count++
			if len(c.Examples) < 3 {
				c.Examples = append(c.Examples, line)
			}
		} else {
			clusters[tmpl] = &TemplateCluster{
				Template: tmpl,
				Count:    1,
				Examples: []string{line},
			}
		}
	}

	var result []TemplateCluster
	for _, c := range clusters {
		result = append(result, *c)
	}

	sort.Slice(result, func(i, j int) bool {
		return result[i].Count > result[j].Count
	})

	return result
}

// FormatTemplateClusters formats the top mined clusters into a summary table.
func FormatTemplateClusters(clusters []TemplateCluster, topN int) string {
	if len(clusters) == 0 {
		return "No log templates mined."
	}
	if topN <= 0 || topN > len(clusters) {
		topN = len(clusters)
	}

	var sb strings.Builder
	sb.WriteString(fmt.Sprintf("Top %d Mined Log Templates:\n", topN))
	for i := 0; i < topN; i++ {
		c := clusters[i]
		sb.WriteString(fmt.Sprintf("  [%d] Count: %d | Template: %s\n", i+1, c.Count, c.Template))
	}
	return strings.TrimRight(sb.String(), "\n")
}
