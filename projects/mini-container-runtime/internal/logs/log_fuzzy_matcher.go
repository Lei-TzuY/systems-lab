// Package logs provides container log processing utilities.
// This file implements a Levenshtein distance and fuzzy search filter
// for approximate string matching in container log streams.

package logs

import (
	"fmt"
	"strings"
)

// LevenshteinDistance calculates the minimum number of single-character edits
// (insertions, deletions, substitutions) between two strings.
func LevenshteinDistance(s1, s2 string) int {
	r1, r2 := []rune(s1), []rune(s2)
	len1, len2 := len(r1), len(r2)

	if len1 == 0 {
		return len2
	}
	if len2 == 0 {
		return len1
	}

	dp := make([][]int, len1+1)
	for i := range dp {
		dp[i] = make([]int, len2+1)
		dp[i][0] = i
	}
	for j := 0; j <= len2; j++ {
		dp[0][j] = j
	}

	for i := 1; i <= len1; i++ {
		for j := 1; j <= len2; j++ {
			cost := 1
			if r1[i-1] == r2[j-1] {
				cost = 0
			}
			dp[i][j] = min3(
				dp[i-1][j]+1,      // deletion
				dp[i][j-1]+1,      // insertion
				dp[i-1][j-1]+cost, // substitution
			)
		}
	}

	return dp[len1][len2]
}

func min3(a, b, c int) int {
	if a < b {
		if a < c {
			return a
		}
		return c
	}
	if b < c {
		return b
	}
	return c
}

// FuzzySimilarityRatio computes similarity ratio between 0.0 and 1.0 (1.0 = exact match).
func FuzzySimilarityRatio(s1, s2 string) float64 {
	maxLen := len([]rune(s1))
	if l := len([]rune(s2)); l > maxLen {
		maxLen = l
	}
	if maxLen == 0 {
		return 1.0
	}
	dist := LevenshteinDistance(s1, s2)
	return 1.0 - (float64(dist) / float64(maxLen))
}

// FuzzyMatcher filters log lines that closely match a target query.
type FuzzyMatcher struct {
	TargetQuery     string
	MinSimilarity   float64
	CaseInsensitive bool
}

// NewFuzzyMatcher creates a FuzzyMatcher.
func NewFuzzyMatcher(targetQuery string, minSimilarity float64, caseInsensitive bool) (*FuzzyMatcher, error) {
	if targetQuery == "" {
		return nil, fmt.Errorf("target query cannot be empty")
	}
	if minSimilarity <= 0 {
		minSimilarity = 0.7
	}
	if minSimilarity > 1.0 {
		minSimilarity = 1.0
	}
	return &FuzzyMatcher{
		TargetQuery:     targetQuery,
		MinSimilarity:   minSimilarity,
		CaseInsensitive: caseInsensitive,
	}, nil
}

// MatchSubstrings checks if any sliding window substring of line matches TargetQuery within similarity threshold.
func (fm *FuzzyMatcher) Match(line string) bool {
	q := fm.TargetQuery
	l := line
	if fm.CaseInsensitive {
		q = strings.ToLower(q)
		l = strings.ToLower(l)
	}

	// Exact substring contains is an immediate match
	if strings.Contains(l, q) {
		return true
	}

	qRunes := []rune(q)
	lRunes := []rune(l)
	qLen := len(qRunes)
	lLen := len(lRunes)

	if lLen < qLen {
		return FuzzySimilarityRatio(q, l) >= fm.MinSimilarity
	}

	// Sliding window match over candidate substring lengths
	for winLen := qLen - 2; winLen <= qLen+2; winLen++ {
		if winLen <= 0 || winLen > lLen {
			continue
		}
		for i := 0; i+winLen <= lLen; i++ {
			sub := string(lRunes[i : i+winLen])
			if FuzzySimilarityRatio(q, sub) >= fm.MinSimilarity {
				return true
			}
		}
	}

	return false
}

// FilterLines returns only lines that fuzzy match the query.
func (fm *FuzzyMatcher) FilterLines(lines []string) []string {
	var result []string
	for _, line := range lines {
		if fm.Match(line) {
			result = append(result, line)
		}
	}
	return result
}
