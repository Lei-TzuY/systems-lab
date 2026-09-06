package network

import (
	"strings"
	"testing"
)

func TestGenerateDNSSingleReqRecheckTimeoutConfig(t *testing.T) {
	tests := []struct {
		name    string
		timeout int
		want    string
	}{
		{
			name:    "custom timeout",
			timeout: 8,
			want:    "options single-request-recheck timeout:8\n",
		},
		{
			name:    "zero timeout defaults to 5",
			timeout: 0,
			want:    "options single-request-recheck timeout:5\n",
		},
		{
			name:    "negative timeout defaults to 5",
			timeout: -1,
			want:    "options single-request-recheck timeout:5\n",
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := GenerateDNSSingleReqRecheckTimeoutConfig(tc.timeout)
			if got != tc.want {
				t.Errorf("got %q, want %q", got, tc.want)
			}
		})
	}
}

func TestGenerateDNSSingleReqRecheckTimeoutConfig_Contents(t *testing.T) {
	got := GenerateDNSSingleReqRecheckTimeoutConfig(4)
	if !strings.Contains(got, "single-request-recheck") {
		t.Errorf("expected single-request-recheck in %q", got)
	}
	if !strings.Contains(got, "timeout:4") {
		t.Errorf("expected timeout:4 in %q", got)
	}
}
