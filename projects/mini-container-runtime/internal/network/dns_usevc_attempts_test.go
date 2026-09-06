package network

import (
	"strings"
	"testing"
)

func TestGenerateDNSUseVCAttemptsConfig(t *testing.T) {
	tests := []struct {
		name     string
		attempts int
		want     string
	}{
		{
			name:     "custom attempts",
			attempts: 5,
			want:     "options use-vc attempts:5\n",
		},
		{
			name:     "zero attempts defaults to 2",
			attempts: 0,
			want:     "options use-vc attempts:2\n",
		},
		{
			name:     "negative attempts defaults to 2",
			attempts: -1,
			want:     "options use-vc attempts:2\n",
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := GenerateDNSUseVCAttemptsConfig(tc.attempts)
			if got != tc.want {
				t.Errorf("GenerateDNSUseVCAttemptsConfig(%d) = %q, want %q", tc.attempts, got, tc.want)
			}
		})
	}
}

func TestGenerateDNSUseVCAttemptsConfig_Contents(t *testing.T) {
	got := GenerateDNSUseVCAttemptsConfig(3)
	if !strings.Contains(got, "use-vc") {
		t.Errorf("expected use-vc in %q", got)
	}
	if !strings.Contains(got, "attempts:3") {
		t.Errorf("expected attempts:3 in %q", got)
	}
}
