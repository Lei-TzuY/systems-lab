package network

import (
	"strings"
	"testing"
)

func TestGenerateDNSRotateAttemptsNDotsConfig(t *testing.T) {
	tests := []struct {
		name     string
		attempts int
		ndots    int
		want     string
	}{
		{
			name:     "custom values",
			attempts: 3,
			ndots:    5,
			want:     "options rotate attempts:3 ndots:5\n",
		},
		{
			name:     "defaults on zero",
			attempts: 0,
			ndots:    0,
			want:     "options rotate attempts:2 ndots:0\n",
		},
		{
			name:     "negative attempts and ndots",
			attempts: -1,
			ndots:    -1,
			want:     "options rotate attempts:2 ndots:1\n",
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := GenerateDNSRotateAttemptsNDotsConfig(tc.attempts, tc.ndots)
			if got != tc.want {
				t.Errorf("got %q, want %q", got, tc.want)
			}
		})
	}
}

func TestGenerateDNSRotateAttemptsNDotsConfig_Contents(t *testing.T) {
	got := GenerateDNSRotateAttemptsNDotsConfig(4, 3)
	for _, kw := range []string{"rotate", "attempts:4", "ndots:3"} {
		if !strings.Contains(got, kw) {
			t.Errorf("expected %q in %q", kw, got)
		}
	}
}
