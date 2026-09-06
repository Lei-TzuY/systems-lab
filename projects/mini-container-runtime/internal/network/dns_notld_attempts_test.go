package network

import (
	"strings"
	"testing"
)

func TestGenerateDNSNoTLDAttemptsConfig(t *testing.T) {
	tests := []struct {
		name     string
		attempts int
		want     string
	}{
		{
			name:     "custom attempts",
			attempts: 3,
			want:     "options no-tld-query attempts:3\n",
		},
		{
			name:     "zero attempts defaults to 2",
			attempts: 0,
			want:     "options no-tld-query attempts:2\n",
		},
		{
			name:     "negative attempts defaults to 2",
			attempts: -1,
			want:     "options no-tld-query attempts:2\n",
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := GenerateDNSNoTLDAttemptsConfig(tc.attempts)
			if got != tc.want {
				t.Errorf("GenerateDNSNoTLDAttemptsConfig(%d) = %q, want %q", tc.attempts, got, tc.want)
			}
		})
	}
}

func TestGenerateDNSNoTLDAttemptsConfig_Contents(t *testing.T) {
	got := GenerateDNSNoTLDAttemptsConfig(5)
	if !strings.Contains(got, "no-tld-query") {
		t.Errorf("expected no-tld-query in %q", got)
	}
	if !strings.Contains(got, "attempts:5") {
		t.Errorf("expected attempts:5 in %q", got)
	}
}
