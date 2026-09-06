package network

import (
	"strings"
	"testing"
)

func TestGenerateDNSNoReloadAttemptsConfig(t *testing.T) {
	tests := []struct {
		name     string
		attempts int
		want     string
	}{
		{
			name:     "custom attempts",
			attempts: 4,
			want:     "options no-reload attempts:4\n",
		},
		{
			name:     "zero attempts defaults to 2",
			attempts: 0,
			want:     "options no-reload attempts:2\n",
		},
		{
			name:     "negative attempts defaults to 2",
			attempts: -1,
			want:     "options no-reload attempts:2\n",
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := GenerateDNSNoReloadAttemptsConfig(tc.attempts)
			if got != tc.want {
				t.Errorf("GenerateDNSNoReloadAttemptsConfig(%d) = %q, want %q", tc.attempts, got, tc.want)
			}
		})
	}
}

func TestGenerateDNSNoReloadAttemptsConfig_Contents(t *testing.T) {
	got := GenerateDNSNoReloadAttemptsConfig(3)
	if !strings.Contains(got, "no-reload") {
		t.Errorf("expected no-reload in %q", got)
	}
	if !strings.Contains(got, "attempts:3") {
		t.Errorf("expected attempts:3 in %q", got)
	}
}
