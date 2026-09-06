package network

import (
	"strings"
	"testing"
)

func TestGenerateDNSTrustADAttemptsConfig(t *testing.T) {
	tests := []struct {
		name     string
		attempts int
		want     string
	}{
		{
			name:     "custom attempts",
			attempts: 3,
			want:     "options trust-ad attempts:3\n",
		},
		{
			name:     "zero attempts defaults to 2",
			attempts: 0,
			want:     "options trust-ad attempts:2\n",
		},
		{
			name:     "negative attempts defaults to 2",
			attempts: -1,
			want:     "options trust-ad attempts:2\n",
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := GenerateDNSTrustADAttemptsConfig(tc.attempts)
			if got != tc.want {
				t.Errorf("GenerateDNSTrustADAttemptsConfig(%d) = %q, want %q", tc.attempts, got, tc.want)
			}
		})
	}
}

func TestGenerateDNSTrustADAttemptsConfig_Contents(t *testing.T) {
	got := GenerateDNSTrustADAttemptsConfig(4)
	if !strings.Contains(got, "trust-ad") {
		t.Errorf("expected trust-ad in %q", got)
	}
	if !strings.Contains(got, "attempts:4") {
		t.Errorf("expected attempts:4 in %q", got)
	}
}
