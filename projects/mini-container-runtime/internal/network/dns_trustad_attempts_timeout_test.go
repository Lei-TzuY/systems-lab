package network

import (
	"strings"
	"testing"
)

func TestGenerateDNSTrustADAttemptsTimeoutConfig(t *testing.T) {
	tests := []struct {
		name     string
		attempts int
		timeout  int
		want     string
	}{
		{
			name:     "custom values",
			attempts: 3,
			timeout:  8,
			want:     "options trust-ad attempts:3 timeout:8\n",
		},
		{
			name:     "defaults on zero",
			attempts: 0,
			timeout:  0,
			want:     "options trust-ad attempts:2 timeout:5\n",
		},
		{
			name:     "negative values",
			attempts: -1,
			timeout:  -1,
			want:     "options trust-ad attempts:2 timeout:5\n",
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := GenerateDNSTrustADAttemptsTimeoutConfig(tc.attempts, tc.timeout)
			if got != tc.want {
				t.Errorf("got %q, want %q", got, tc.want)
			}
		})
	}
}

func TestGenerateDNSTrustADAttemptsTimeoutConfig_Contents(t *testing.T) {
	got := GenerateDNSTrustADAttemptsTimeoutConfig(4, 10)
	for _, kw := range []string{"trust-ad", "attempts:4", "timeout:10"} {
		if !strings.Contains(got, kw) {
			t.Errorf("expected %q in %q", kw, got)
		}
	}
}
