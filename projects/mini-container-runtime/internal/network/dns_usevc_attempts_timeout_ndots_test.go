package network

import (
	"strings"
	"testing"
)

func TestGenerateDNSUseVCAttemptsTimeoutNdotsConfig(t *testing.T) {
	tests := []struct {
		name     string
		attempts int
		timeout  int
		ndots    int
		want     string
	}{
		{
			name:     "custom values",
			attempts: 3,
			timeout:  7,
			ndots:    2,
			want:     "options use-vc attempts:3 timeout:7 ndots:2\n",
		},
		{
			name:     "zero values are explicit",
			attempts: 0,
			timeout:  0,
			ndots:    0,
			want:     "options use-vc attempts:0 timeout:0 ndots:0\n",
		},
		{
			name:     "negative values request defaults",
			attempts: -1,
			timeout:  -1,
			ndots:    -1,
			want:     "options use-vc attempts:2 timeout:5 ndots:1\n",
		},
		{
			name:     "values above glibc caps are clamped",
			attempts: 99,
			timeout:  99,
			ndots:    99,
			want:     "options use-vc attempts:5 timeout:30 ndots:15\n",
		},
		{
			name:     "boundary values are preserved",
			attempts: 5,
			timeout:  30,
			ndots:    15,
			want:     "options use-vc attempts:5 timeout:30 ndots:15\n",
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := GenerateDNSUseVCAttemptsTimeoutNdotsConfig(tc.attempts, tc.timeout, tc.ndots)
			if got != tc.want {
				t.Errorf("got %q, want %q", got, tc.want)
			}
		})
	}
}

func TestGenerateDNSUseVCAttemptsTimeoutNdotsConfig_Contents(t *testing.T) {
	got := GenerateDNSUseVCAttemptsTimeoutNdotsConfig(4, 15, 3)
	for _, kw := range []string{"use-vc", "attempts:4", "timeout:15", "ndots:3"} {
		if !strings.Contains(got, kw) {
			t.Errorf("expected %q in %q", kw, got)
		}
	}
}

func TestNormalizeResolverOption(t *testing.T) {
	if got := normalizeResolverOption(-1, 2, 5); got != 2 {
		t.Fatalf("negative value normalized to %d, want default 2", got)
	}
	if got := normalizeResolverOption(0, 2, 5); got != 0 {
		t.Fatalf("zero normalized to %d, want explicit zero", got)
	}
	if got := normalizeResolverOption(6, 2, 5); got != 5 {
		t.Fatalf("overflow normalized to %d, want cap 5", got)
	}
}
