package network

import (
	"strings"
	"testing"
)

func TestGenerateDNSTrustADAttemptsTimeoutNdotsConfig(t *testing.T) {
	tests := []struct {
		name     string
		attempts int
		timeout  int
		ndots    int
		want     string
	}{
		{
			name:     "custom valid values",
			attempts: 4,
			timeout:  6,
			ndots:    3,
			want:     "options trust-ad attempts:4 timeout:6 ndots:3\n",
		},
		{
			name:     "zero values are explicit",
			attempts: 0,
			timeout:  0,
			ndots:    0,
			want:     "options trust-ad attempts:0 timeout:0 ndots:0\n",
		},
		{
			name:     "negative values request defaults",
			attempts: -1,
			timeout:  -1,
			ndots:    -1,
			want:     "options trust-ad attempts:2 timeout:5 ndots:1\n",
		},
		{
			name:     "values above glibc caps are clamped",
			attempts: 100,
			timeout:  50,
			ndots:    20,
			want:     "options trust-ad attempts:5 timeout:30 ndots:15\n",
		},
		{
			name:     "boundary values are preserved",
			attempts: 5,
			timeout:  30,
			ndots:    15,
			want:     "options trust-ad attempts:5 timeout:30 ndots:15\n",
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := GenerateDNSTrustADAttemptsTimeoutNdotsConfig(tc.attempts, tc.timeout, tc.ndots)
			if got != tc.want {
				t.Errorf("got %q, want %q", got, tc.want)
			}
		})
	}
}

func TestGenerateDNSTrustADAttemptsTimeoutNdotsConfig_Contents(t *testing.T) {
	got := GenerateDNSTrustADAttemptsTimeoutNdotsConfig(5, 12, 4)
	for _, kw := range []string{"trust-ad", "attempts:5", "timeout:12", "ndots:4"} {
		if !strings.Contains(got, kw) {
			t.Errorf("expected %q in %q", kw, got)
		}
	}
}
