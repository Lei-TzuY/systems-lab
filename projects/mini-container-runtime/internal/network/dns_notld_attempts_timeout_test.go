package network

import (
	"strings"
	"testing"
)

func TestGenerateDNSNoTLDAttemptsTimeoutConfig(t *testing.T) {
	tests := []struct {
		name     string
		attempts int
		timeout  int
		want     string
	}{
		{
			name:     "custom values",
			attempts: 5,
			timeout:  8,
			want:     "options no-tld-query attempts:5 timeout:8\n",
		},
		{
			name:     "defaults on zero",
			attempts: 0,
			timeout:  0,
			want:     "options no-tld-query attempts:2 timeout:5\n",
		},
		{
			name:     "negative values",
			attempts: -1,
			timeout:  -1,
			want:     "options no-tld-query attempts:2 timeout:5\n",
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := GenerateDNSNoTLDAttemptsTimeoutConfig(tc.attempts, tc.timeout)
			if got != tc.want {
				t.Errorf("got %q, want %q", got, tc.want)
			}
		})
	}
}

func TestGenerateDNSNoTLDAttemptsTimeoutConfig_Contents(t *testing.T) {
	got := GenerateDNSNoTLDAttemptsTimeoutConfig(3, 10)
	for _, kw := range []string{"no-tld-query", "attempts:3", "timeout:10"} {
		if !strings.Contains(got, kw) {
			t.Errorf("expected %q in %q", kw, got)
		}
	}
}
