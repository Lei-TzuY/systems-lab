package network

import (
	"strings"
	"testing"
)

func TestGenerateDNSUseVCAttemptsTimeoutConfig(t *testing.T) {
	tests := []struct {
		name     string
		attempts int
		timeout  int
		want     string
	}{
		{
			name:     "custom values",
			attempts: 4,
			timeout:  6,
			want:     "options use-vc attempts:4 timeout:6\n",
		},
		{
			name:     "defaults on zero",
			attempts: 0,
			timeout:  0,
			want:     "options use-vc attempts:2 timeout:5\n",
		},
		{
			name:     "negative values",
			attempts: -1,
			timeout:  -1,
			want:     "options use-vc attempts:2 timeout:5\n",
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := GenerateDNSUseVCAttemptsTimeoutConfig(tc.attempts, tc.timeout)
			if got != tc.want {
				t.Errorf("got %q, want %q", got, tc.want)
			}
		})
	}
}

func TestGenerateDNSUseVCAttemptsTimeoutConfig_Contents(t *testing.T) {
	got := GenerateDNSUseVCAttemptsTimeoutConfig(5, 12)
	for _, kw := range []string{"use-vc", "attempts:5", "timeout:12"} {
		if !strings.Contains(got, kw) {
			t.Errorf("expected %q in %q", kw, got)
		}
	}
}
