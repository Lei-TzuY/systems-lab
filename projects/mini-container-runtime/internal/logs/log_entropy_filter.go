// Package logs provides container log processing utilities.
// This file implements a Shannon entropy calculator and anomaly filter
// to detect high-entropy data (e.g., base64 tokens, secret keys, encrypted payloads) in container logs.

package logs

import (
	"fmt"
	"math"
)

// CalculateEntropy calculates the Shannon entropy (in bits per character) of a string.
// Range: 0.0 (uniform single character) to ~8.0 (uniformly distributed byte space).
func CalculateEntropy(s string) float64 {
	if len(s) == 0 {
		return 0.0
	}

	charCounts := make(map[rune]int)
	for _, r := range s {
		charCounts[r]++
	}

	total := float64(len([]rune(s)))
	entropy := 0.0
	for _, count := range charCounts {
		p := float64(count) / total
		entropy -= p * math.Log2(p)
	}

	return entropy
}

// EntropyFilter filters log lines based on Shannon entropy thresholds.
type EntropyFilter struct {
	MinEntropy float64
	MaxEntropy float64
}

// NewEntropyFilter creates an EntropyFilter with min and max entropy thresholds.
func NewEntropyFilter(minEntropy, maxEntropy float64) *EntropyFilter {
	if maxEntropy <= 0 {
		maxEntropy = 8.0
	}
	return &EntropyFilter{
		MinEntropy: minEntropy,
		MaxEntropy: maxEntropy,
	}
}

// Match returns true if the line's Shannon entropy falls within [MinEntropy, MaxEntropy].
func (ef *EntropyFilter) Match(line string) bool {
	ent := CalculateEntropy(line)
	return ent >= ef.MinEntropy && ent <= ef.MaxEntropy
}

// FilterLines processes a slice of log lines and returns only lines matching the entropy criteria.
func (ef *EntropyFilter) FilterLines(lines []string) []string {
	var result []string
	for _, line := range lines {
		if ef.Match(line) {
			result = append(result, line)
		}
	}
	return result
}

// FormatEntropyStats formats line text with its calculated Shannon entropy.
func FormatEntropyStats(line string) string {
	ent := CalculateEntropy(line)
	return fmt.Sprintf("[entropy: %.2f] %s", ent, line)
}
