package network

import (
	"strings"
	"testing"
)

func TestGenerateDNSSingleReqRecheckAttemptsConfig(t *testing.T) {
	tests := []struct {
		name     string
		attempts int
		want     string
	}{
		{
			name:     "custom attempts",
			attempts: 4,
			want:     "options single-request-recheck attempts:4\n",
		},
		{
			name:     "zero attempts defaults to 2",
			attempts: 0,
			want:     "options single-request-recheck attempts:2\n",
		},
		{
			name:     "negative attempts defaults to 2",
			attempts: -1,
			want:     "options single-request-recheck attempts:2\n",
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := GenerateDNSSingleReqRecheckAttemptsConfig(tc.attempts)
			if got != tc.want {
				t.Errorf("got %q, want %q", got, tc.want)
			}
		})
	}
}

func TestGenerateDNSSingleReqRecheckAttemptsConfig_Contents(t *testing.T) {
	got := GenerateDNSSingleReqRecheckAttemptsConfig(3)
	if !strings.Contains(got, "single-request-recheck") {
		t.Errorf("expected single-request-recheck in %q", got)
	}
	if !strings.Contains(got, "attempts:3") {
		t.Errorf("expected attempts:3 in %q", got)
	}
}
