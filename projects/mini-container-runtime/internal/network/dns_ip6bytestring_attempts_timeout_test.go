package network

import (
	"strings"
	"testing"
)

func TestGenerateDNSIP6BytestringAttemptsTimeoutConfig(t *testing.T) {
	tests := []struct {
		name     string
		attempts int
		timeout  int
		want     string
	}{
		{
			name:     "custom values",
			attempts: 5,
			timeout:  12,
			want:     "options ip6-bytestring attempts:5 timeout:12\n",
		},
		{
			name:     "defaults on zero",
			attempts: 0,
			timeout:  0,
			want:     "options ip6-bytestring attempts:2 timeout:5\n",
		},
		{
			name:     "negative values",
			attempts: -1,
			timeout:  -1,
			want:     "options ip6-bytestring attempts:2 timeout:5\n",
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := GenerateDNSIP6BytestringAttemptsTimeoutConfig(tc.attempts, tc.timeout)
			if got != tc.want {
				t.Errorf("got %q, want %q", got, tc.want)
			}
		})
	}
}

func TestGenerateDNSIP6BytestringAttemptsTimeoutConfig_Contents(t *testing.T) {
	got := GenerateDNSIP6BytestringAttemptsTimeoutConfig(3, 8)
	for _, kw := range []string{"ip6-bytestring", "attempts:3", "timeout:8"} {
		if !strings.Contains(got, kw) {
			t.Errorf("expected %q in %q", kw, got)
		}
	}
}
