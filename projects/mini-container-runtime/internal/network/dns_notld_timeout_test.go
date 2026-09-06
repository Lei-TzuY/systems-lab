package network

import (
	"strings"
	"testing"
)

func TestGenerateDNSNoTLDTimeoutConfig(t *testing.T) {
	tests := []struct {
		name    string
		timeout int
		want    string
	}{
		{
			name:    "positive timeout",
			timeout: 3,
			want:    "options no-tld-query timeout:3\n",
		},
		{
			name:    "zero timeout defaults to 5",
			timeout: 0,
			want:    "options no-tld-query timeout:5\n",
		},
		{
			name:    "negative timeout defaults to 5",
			timeout: -1,
			want:    "options no-tld-query timeout:5\n",
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := GenerateDNSNoTLDTimeoutConfig(tc.timeout)
			if got != tc.want {
				t.Errorf("GenerateDNSNoTLDTimeoutConfig(%d) = %q, want %q", tc.timeout, got, tc.want)
			}
		})
	}
}

func TestGenerateDNSNoTLDTimeoutConfig_Contents(t *testing.T) {
	got := GenerateDNSNoTLDTimeoutConfig(7)
	if !strings.Contains(got, "no-tld-query") {
		t.Errorf("expected no-tld-query in %q", got)
	}
	if !strings.Contains(got, "timeout:7") {
		t.Errorf("expected timeout:7 in %q", got)
	}
}
