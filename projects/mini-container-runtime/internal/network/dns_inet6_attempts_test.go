package network

import (
	"strings"
	"testing"
)

func TestGenerateDNSInet6AttemptsConfig(t *testing.T) {
	tests := []struct {
		name     string
		attempts int
		want     string
	}{
		{
			name:     "custom attempts",
			attempts: 3,
			want:     "options inet6 attempts:3\n",
		},
		{
			name:     "zero attempts defaults to 2",
			attempts: 0,
			want:     "options inet6 attempts:2\n",
		},
		{
			name:     "negative attempts defaults to 2",
			attempts: -1,
			want:     "options inet6 attempts:2\n",
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := GenerateDNSInet6AttemptsConfig(tc.attempts)
			if got != tc.want {
				t.Errorf("got %q, want %q", got, tc.want)
			}
		})
	}
}

func TestGenerateDNSInet6AttemptsConfig_Contents(t *testing.T) {
	got := GenerateDNSInet6AttemptsConfig(4)
	if !strings.Contains(got, "inet6") {
		t.Errorf("expected inet6 in %q", got)
	}
	if !strings.Contains(got, "attempts:4") {
		t.Errorf("expected attempts:4 in %q", got)
	}
}
