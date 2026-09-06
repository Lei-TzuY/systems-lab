package network

import (
	"strings"
	"testing"
)

func TestGenerateDNSEDNS0AttemptsTimeoutConfig(t *testing.T) {
	tests := []struct {
		name     string
		attempts int
		timeout  int
		want     string
	}{
		{
			name:     "custom values",
			attempts: 4,
			timeout:  7,
			want:     "options edns0 attempts:4 timeout:7\n",
		},
		{
			name:     "defaults on zero",
			attempts: 0,
			timeout:  0,
			want:     "options edns0 attempts:2 timeout:5\n",
		},
		{
			name:     "negative values",
			attempts: -1,
			timeout:  -1,
			want:     "options edns0 attempts:2 timeout:5\n",
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := GenerateDNSEDNS0AttemptsTimeoutConfig(tc.attempts, tc.timeout)
			if got != tc.want {
				t.Errorf("got %q, want %q", got, tc.want)
			}
		})
	}
}

func TestGenerateDNSEDNS0AttemptsTimeoutConfig_Contents(t *testing.T) {
	got := GenerateDNSEDNS0AttemptsTimeoutConfig(3, 9)
	for _, kw := range []string{"edns0", "attempts:3", "timeout:9"} {
		if !strings.Contains(got, kw) {
			t.Errorf("expected %q in %q", kw, got)
		}
	}
}
