package logs

import (
	"strings"
	"testing"
)

func TestIPMasker_MaskLastOctet(t *testing.T) {
	masker := NewIPMasker(MaskLastOctet)

	tests := []struct {
		name     string
		input    string
		wantMask string
	}{
		{
			name:     "single ipv4",
			input:    "Client connected from 192.168.1.50 on port 8080",
			wantMask: "192.168.1.xxx",
		},
		{
			name:     "ipv6 address",
			input:    "Connection from 2001:0db8:85a3:0000:0000:8a2e:0370:7334 accepted",
			wantMask: "[IPv6_MASKED]",
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := masker.MaskLine(tc.input)
			if !strings.Contains(got, tc.wantMask) {
				t.Errorf("got %q, want containing %q", got, tc.wantMask)
			}
		})
	}
}

func TestIPMasker_MaskFull(t *testing.T) {
	masker := NewIPMasker(MaskFull)
	got := masker.MaskLine("Request forwarded to 10.0.0.15")
	if !strings.Contains(got, "[IPv4_MASKED]") {
		t.Errorf("got %q, want [IPv4_MASKED]", got)
	}
}
