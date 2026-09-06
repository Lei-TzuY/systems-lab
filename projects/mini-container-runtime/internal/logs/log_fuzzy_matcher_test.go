package logs

import (
	"testing"
)

func TestLevenshteinDistance(t *testing.T) {
	tests := []struct {
		s1, s2 string
		want   int
	}{
		{"kitten", "sitting", 3},
		{"rosettacode", "raisethysword", 8},
		{"same", "same", 0},
		{"", "abc", 3},
		{"abc", "", 3},
	}

	for _, tc := range tests {
		t.Run(tc.s1+"_"+tc.s2, func(t *testing.T) {
			got := LevenshteinDistance(tc.s1, tc.s2)
			if got != tc.want {
				t.Errorf("LevenshteinDistance(%q, %q) = %d, want %d", tc.s1, tc.s2, got, tc.want)
			}
		})
	}
}

func TestFuzzyMatcher_Match(t *testing.T) {
	matcher, err := NewFuzzyMatcher("Connection refused", 0.75, true)
	if err != nil {
		t.Fatalf("NewFuzzyMatcher failed: %v", err)
	}

	tests := []struct {
		line string
		want bool
	}{
		{"2026-08-20 [ERROR] Connection refused by host 10.0.0.1", true},
		{"2026-08-20 [ERROR] Connectin refused by host", true}, // typo in Connection
		{"2026-08-20 [INFO] System is running normally", false},
	}

	for _, tc := range tests {
		t.Run(tc.line, func(t *testing.T) {
			got := matcher.Match(tc.line)
			if got != tc.want {
				t.Errorf("Match(%q) = %t, want %t", tc.line, got, tc.want)
			}
		})
	}
}

func TestFuzzyMatcher_Validation(t *testing.T) {
	if _, err := NewFuzzyMatcher("", 0.8, true); err == nil {
		t.Error("expected error for empty target query")
	}
}
