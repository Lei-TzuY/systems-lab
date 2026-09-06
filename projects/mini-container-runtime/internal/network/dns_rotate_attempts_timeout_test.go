package network

import (
	"strings"
	"testing"
)

func TestGenerateDNSRotateAttemptsTimeoutConfig(t *testing.T) {
	tests := []struct {
		name     string
		attempts int
		timeout  int
		want     string
	}{
		{
			name:     "custom values",
			attempts: 4,
			timeout:  10,
			want:     "options rotate attempts:4 timeout:10\n",
		},
		{
			name:     "defaults on zero",
			attempts: 0,
			timeout:  0,
			want:     "options rotate attempts:2 timeout:5\n",
		},
		{
			name:     "negative values",
			attempts: -1,
			timeout:  -1,
			want:     "options rotate attempts:2 timeout:5\n",
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := GenerateDNSRotateAttemptsTimeoutConfig(tc.attempts, tc.timeout)
			if got != tc.want {
				t.Errorf("got %q, want %q", got, tc.want)
			}
		})
	}
}

func TestGenerateDNSRotateAttemptsTimeoutConfig_Contents(t *testing.T) {
	got := GenerateDNSRotateAttemptsTimeoutConfig(3, 9)
	for _, kw := range []string{"rotate", "attempts:3", "timeout:9"} {
		if !strings.Contains(got, kw) {
			t.Errorf("expected %q in %q", kw, got)
		}
	}
}
