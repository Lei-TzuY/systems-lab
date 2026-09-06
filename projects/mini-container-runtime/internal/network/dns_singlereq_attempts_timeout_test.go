package network

import (
	"strings"
	"testing"
)

func TestGenerateDNSSingleReqAttemptsTimeoutConfig(t *testing.T) {
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
			want:     "options single-request attempts:3 timeout:6\n",
		},
		{
			name:     "defaults on zero",
			attempts: 0,
			timeout:  0,
			want:     "options single-request attempts:2 timeout:5\n",
		},
		{
			name:     "negative values",
			attempts: -1,
			timeout:  -1,
			want:     "options single-request attempts:2 timeout:5\n",
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := GenerateDNSSingleReqAttemptsTimeoutConfig(tc.attempts, tc.timeout)
			if got != tc.want {
				t.Errorf("got %q, want %q", got, tc.want)
			}
		})
	}
}

func TestGenerateDNSSingleReqAttemptsTimeoutConfig_Contents(t *testing.T) {
	got := GenerateDNSSingleReqAttemptsTimeoutConfig(4, 8)
	for _, kw := range []string{"single-request", "attempts:4", "timeout:8"} {
		if !strings.Contains(got, kw) {
			t.Errorf("expected %q in %q", kw, got)
		}
	}
}
