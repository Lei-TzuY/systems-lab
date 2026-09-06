package network

import (
	"strings"
	"testing"
)

func TestGenerateDNSInet6AttemptsTimeoutConfig(t *testing.T) {
	tests := []struct {
		name     string
		attempts int
		timeout  int
		want     string
	}{
		{
			name:     "custom values",
			attempts: 3,
			timeout:  4,
			want:     "options inet6 attempts:3 timeout:4\n",
		},
		{
			name:     "defaults on zero",
			attempts: 0,
			timeout:  0,
			want:     "options inet6 attempts:2 timeout:5\n",
		},
		{
			name:     "negative values",
			attempts: -1,
			timeout:  -1,
			want:     "options inet6 attempts:2 timeout:5\n",
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := GenerateDNSInet6AttemptsTimeoutConfig(tc.attempts, tc.timeout)
			if got != tc.want {
				t.Errorf("got %q, want %q", got, tc.want)
			}
		})
	}
}

func TestGenerateDNSInet6AttemptsTimeoutConfig_Contents(t *testing.T) {
	got := GenerateDNSInet6AttemptsTimeoutConfig(4, 8)
	for _, kw := range []string{"inet6", "attempts:4", "timeout:8"} {
		if !strings.Contains(got, kw) {
			t.Errorf("expected %q in %q", kw, got)
		}
	}
}
