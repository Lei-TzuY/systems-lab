package network

import (
	"strings"
	"testing"
)

func TestGenerateDNSNoReloadAttemptsTimeoutConfig(t *testing.T) {
	tests := []struct {
		name     string
		attempts int
		timeout  int
		want     string
	}{
		{
			name:     "custom values",
			attempts: 3,
			timeout:  6,
			want:     "options no-reload attempts:3 timeout:6\n",
		},
		{
			name:     "defaults on zero",
			attempts: 0,
			timeout:  0,
			want:     "options no-reload attempts:2 timeout:5\n",
		},
		{
			name:     "negative values",
			attempts: -1,
			timeout:  -1,
			want:     "options no-reload attempts:2 timeout:5\n",
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := GenerateDNSNoReloadAttemptsTimeoutConfig(tc.attempts, tc.timeout)
			if got != tc.want {
				t.Errorf("got %q, want %q", got, tc.want)
			}
		})
	}
}

func TestGenerateDNSNoReloadAttemptsTimeoutConfig_Contents(t *testing.T) {
	got := GenerateDNSNoReloadAttemptsTimeoutConfig(4, 9)
	for _, kw := range []string{"no-reload", "attempts:4", "timeout:9"} {
		if !strings.Contains(got, kw) {
			t.Errorf("expected %q in %q", kw, got)
		}
	}
}
