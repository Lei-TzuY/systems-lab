package network

import (
	"strings"
	"testing"
)

func TestGenerateDNSUseVCTimeoutConfig(t *testing.T) {
	tests := []struct {
		name    string
		timeout int
		want    string
	}{
		{
			name:    "positive timeout",
			timeout: 2,
			want:    "options use-vc timeout:2\n",
		},
		{
			name:    "zero timeout defaults to 5",
			timeout: 0,
			want:    "options use-vc timeout:5\n",
		},
		{
			name:    "negative timeout defaults to 5",
			timeout: -3,
			want:    "options use-vc timeout:5\n",
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := GenerateDNSUseVCTimeoutConfig(tc.timeout)
			if got != tc.want {
				t.Errorf("GenerateDNSUseVCTimeoutConfig(%d) = %q, want %q", tc.timeout, got, tc.want)
			}
		})
	}
}

func TestGenerateDNSUseVCTimeoutConfig_Contents(t *testing.T) {
	got := GenerateDNSUseVCTimeoutConfig(10)
	if !strings.Contains(got, "use-vc") {
		t.Errorf("expected use-vc in %q", got)
	}
	if !strings.Contains(got, "timeout:10") {
		t.Errorf("expected timeout:10 in %q", got)
	}
}
