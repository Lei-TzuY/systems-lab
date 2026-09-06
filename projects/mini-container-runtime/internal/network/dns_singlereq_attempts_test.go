package network

import (
	"strings"
	"testing"
)

func TestGenerateDNSSingleReqAttemptsConfig(t *testing.T) {
	tests := []struct {
		name     string
		attempts int
		want     string
	}{
		{
			name:     "custom attempts",
			attempts: 4,
			want:     "options single-request attempts:4\n",
		},
		{
			name:     "zero attempts defaults to 2",
			attempts: 0,
			want:     "options single-request attempts:2\n",
		},
		{
			name:     "negative attempts defaults to 2",
			attempts: -1,
			want:     "options single-request attempts:2\n",
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := GenerateDNSSingleReqAttemptsConfig(tc.attempts)
			if got != tc.want {
				t.Errorf("GenerateDNSSingleReqAttemptsConfig(%d) = %q, want %q", tc.attempts, got, tc.want)
			}
		})
	}
}

func TestGenerateDNSSingleReqAttemptsConfig_Contents(t *testing.T) {
	got := GenerateDNSSingleReqAttemptsConfig(5)
	if !strings.Contains(got, "single-request") {
		t.Errorf("expected single-request in %q", got)
	}
	if !strings.Contains(got, "attempts:5") {
		t.Errorf("expected attempts:5 in %q", got)
	}
}
