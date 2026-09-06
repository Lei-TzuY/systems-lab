package network

import (
	"strings"
	"testing"
)

func TestGenerateDNSNoCheckAttemptsTimeoutConfig(t *testing.T) {
	tests := []struct {
		name     string
		attempts int
		timeout  int
		want     string
	}{
		{
			name:     "custom values",
			attempts: 4,
			timeout:  9,
			want:     "options no-check-names attempts:4 timeout:9\n",
		},
		{
			name:     "defaults on zero",
			attempts: 0,
			timeout:  0,
			want:     "options no-check-names attempts:2 timeout:5\n",
		},
		{
			name:     "negative values",
			attempts: -1,
			timeout:  -1,
			want:     "options no-check-names attempts:2 timeout:5\n",
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := GenerateDNSNoCheckAttemptsTimeoutConfig(tc.attempts, tc.timeout)
			if got != tc.want {
				t.Errorf("got %q, want %q", got, tc.want)
			}
		})
	}
}

func TestGenerateDNSNoCheckAttemptsTimeoutConfig_Contents(t *testing.T) {
	got := GenerateDNSNoCheckAttemptsTimeoutConfig(3, 11)
	for _, kw := range []string{"no-check-names", "attempts:3", "timeout:11"} {
		if !strings.Contains(got, kw) {
			t.Errorf("expected %q in %q", kw, got)
		}
	}
}
