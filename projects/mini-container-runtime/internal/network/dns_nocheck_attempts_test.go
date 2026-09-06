package network

import (
	"strings"
	"testing"
)

func TestGenerateDNSNoCheckAttemptsConfig(t *testing.T) {
	tests := []struct {
		name     string
		attempts int
		want     string
	}{
		{
			name:     "custom attempts",
			attempts: 5,
			want:     "options no-check-names attempts:5\n",
		},
		{
			name:     "zero attempts defaults to 2",
			attempts: 0,
			want:     "options no-check-names attempts:2\n",
		},
		{
			name:     "negative attempts defaults to 2",
			attempts: -1,
			want:     "options no-check-names attempts:2\n",
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := GenerateDNSNoCheckAttemptsConfig(tc.attempts)
			if got != tc.want {
				t.Errorf("GenerateDNSNoCheckAttemptsConfig(%d) = %q, want %q", tc.attempts, got, tc.want)
			}
		})
	}
}

func TestGenerateDNSNoCheckAttemptsConfig_Contents(t *testing.T) {
	got := GenerateDNSNoCheckAttemptsConfig(3)
	if !strings.Contains(got, "no-check-names") {
		t.Errorf("expected no-check-names in %q", got)
	}
	if !strings.Contains(got, "attempts:3") {
		t.Errorf("expected attempts:3 in %q", got)
	}
}
