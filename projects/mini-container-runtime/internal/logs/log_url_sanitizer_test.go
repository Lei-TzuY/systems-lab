package logs

import (
	"strings"
	"testing"
)

func TestURLSanitizer_SanitizeURL(t *testing.T) {
	sanitizer := NewURLSanitizer([]string{"session_id"})

	tests := []struct {
		name     string
		input    string
		wantMask string
	}{
		{
			name:     "basic auth password stripped",
			input:    "Connecting to https://admin:superSecretPass@api.internal.net/v1/data",
			wantMask: "https://admin:REDACTED@api.internal.net/v1/data",
		},
		{
			name:     "query params masked",
			input:    "Request to https://example.com/api?token=secretToken123&user=john",
			wantMask: "token=%5BREDACTED%5D",
		},
		{
			name:     "custom sensitive param masked",
			input:    "Fetch https://example.com/auth?session_id=abc123xyz",
			wantMask: "session_id=%5BREDACTED%5D",
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := sanitizer.SanitizeLine(tc.input)
			if !strings.Contains(got, tc.wantMask) {
				t.Errorf("got %q, want containing %q", got, tc.wantMask)
			}
		})
	}
}

func TestURLSanitizer_NonURLUnchanged(t *testing.T) {
	sanitizer := NewURLSanitizer(nil)
	input := "Normal log message with no URLs"
	got := sanitizer.SanitizeLine(input)
	if got != input {
		t.Errorf("got %q, want %q", got, input)
	}
}
