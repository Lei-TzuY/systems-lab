package network

import (
	"strings"
	"testing"
)

func TestGenerateDNSEDNS0AttemptsTimeoutNdotsConfig(t *testing.T) {
	tests := []struct {
		name     string
		attempts int
		timeout  int
		ndots    int
		want     string
	}{
		{
			name:     "custom valid values",
			attempts: 3,
			timeout:  8,
			ndots:    4,
			want:     "options edns0 attempts:3 timeout:8 ndots:4\n",
		},
		{
			name:     "zero values are explicit",
			attempts: 0,
			timeout:  0,
			ndots:    0,
			want:     "options edns0 attempts:0 timeout:0 ndots:0\n",
		},
		{
			name:     "negative values request defaults",
			attempts: -1,
			timeout:  -1,
			ndots:    -1,
			want:     "options edns0 attempts:2 timeout:5 ndots:1\n",
		},
		{
			name:     "values above glibc caps are clamped",
			attempts: 99,
			timeout:  99,
			ndots:    99,
			want:     "options edns0 attempts:5 timeout:30 ndots:15\n",
		},
		{
			name:     "boundary values are preserved",
			attempts: 5,
			timeout:  30,
			ndots:    15,
			want:     "options edns0 attempts:5 timeout:30 ndots:15\n",
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := GenerateDNSEDNS0AttemptsTimeoutNdotsConfig(tc.attempts, tc.timeout, tc.ndots)
			if got != tc.want {
				t.Errorf("got %q, want %q", got, tc.want)
			}
		})
	}
}

func TestGenerateDNSEDNS0AttemptsTimeoutNdotsConfig_Contents(t *testing.T) {
	got := GenerateDNSEDNS0AttemptsTimeoutNdotsConfig(4, 10, 5)
	for _, kw := range []string{"edns0", "attempts:4", "timeout:10", "ndots:5"} {
		if !strings.Contains(got, kw) {
			t.Errorf("expected %q in %q", kw, got)
		}
	}
}
