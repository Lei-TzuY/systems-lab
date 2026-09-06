package network

import (
	"strings"
	"testing"
)

func TestGenerateDNSNdotsTimeoutConfig(t *testing.T) {
	tests := []struct {
		name    string
		ndots   int
		timeout int
		want    string
	}{
		{
			name:    "default ndots and timeout",
			ndots:   1,
			timeout: 5,
			want:    "options ndots:1 timeout:5\n",
		},
		{
			name:    "custom ndots and timeout",
			ndots:   3,
			timeout: 10,
			want:    "options ndots:3 timeout:10\n",
		},
		{
			name:    "zero ndots is valid",
			ndots:   0,
			timeout: 2,
			want:    "options ndots:0 timeout:2\n",
		},
		{
			name:    "negative ndots defaults to 1",
			ndots:   -1,
			timeout: 5,
			want:    "options ndots:1 timeout:5\n",
		},
		{
			name:    "zero timeout defaults to 5",
			ndots:   2,
			timeout: 0,
			want:    "options ndots:2 timeout:5\n",
		},
		{
			name:    "negative timeout defaults to 5",
			ndots:   4,
			timeout: -3,
			want:    "options ndots:4 timeout:5\n",
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := GenerateDNSNdotsTimeoutConfig(tc.ndots, tc.timeout)
			if got != tc.want {
				t.Errorf("got %q, want %q", got, tc.want)
			}
		})
	}
}

func TestGenerateDNSNdotsTimeoutConfig_ContainsNdots(t *testing.T) {
	got := GenerateDNSNdotsTimeoutConfig(5, 3)
	if !strings.Contains(got, "ndots:5") {
		t.Errorf("expected ndots:5 in %q", got)
	}
	if !strings.Contains(got, "timeout:3") {
		t.Errorf("expected timeout:3 in %q", got)
	}
}
