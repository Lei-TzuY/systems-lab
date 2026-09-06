package network

import (
	"strings"
	"testing"
)

func TestGenerateDNSTrustADTimeoutConfig(t *testing.T) {
	tests := []struct {
		name    string
		timeout int
		want    string
	}{
		{
			name:    "positive timeout",
			timeout: 4,
			want:    "options trust-ad timeout:4\n",
		},
		{
			name:    "zero timeout defaults to 5",
			timeout: 0,
			want:    "options trust-ad timeout:5\n",
		},
		{
			name:    "negative timeout defaults to 5",
			timeout: -2,
			want:    "options trust-ad timeout:5\n",
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := GenerateDNSTrustADTimeoutConfig(tc.timeout)
			if got != tc.want {
				t.Errorf("GenerateDNSTrustADTimeoutConfig(%d) = %q, want %q", tc.timeout, got, tc.want)
			}
		})
	}
}

func TestGenerateDNSTrustADTimeoutConfig_Contents(t *testing.T) {
	got := GenerateDNSTrustADTimeoutConfig(8)
	if !strings.Contains(got, "trust-ad") {
		t.Errorf("expected trust-ad in %q", got)
	}
	if !strings.Contains(got, "timeout:8") {
		t.Errorf("expected timeout:8 in %q", got)
	}
}
