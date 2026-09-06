package logs

import (
	"strings"
	"testing"
)

func TestLogRedactor_DefaultRules(t *testing.T) {
	redactor := NewDefaultLogRedactor()

	tests := []struct {
		name     string
		input    string
		wantMask string
	}{
		{
			name:     "bearer token redacted",
			input:    "GET /api/v1 Authorization: Bearer secret_token_value_12345",
			wantMask: "Bearer [REDACTED_TOKEN]",
		},
		{
			name:     "password query param redacted",
			input:    "connect db user=admin password=mySuperSecretPass123 port=5432",
			wantMask: "password=[REDACTED]",
		},
		{
			name:     "email address redacted",
			input:    "User john.doe@example.com logged in",
			wantMask: "[REDACTED_EMAIL]",
		},
		{
			name:     "jwt token redacted",
			input:    "auth header: eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgN_x_w_w1234567890",
			wantMask: "[REDACTED_JWT]",
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := redactor.RedactLine(tc.input)
			if !strings.Contains(got, tc.wantMask) {
				t.Errorf("got %q, expected mask %q in output", got, tc.wantMask)
			}
		})
	}
}

func TestLogRedactor_CustomRule(t *testing.T) {
	redactor := &LogRedactor{}
	if err := redactor.AddRule("SSN", `\d{3}-\d{2}-\d{4}`, "[SSN_MASKED]"); err != nil {
		t.Fatalf("AddRule failed: %v", err)
	}

	got := redactor.RedactLine("Customer SSN is 123-45-6789.")
	if !strings.Contains(got, "[SSN_MASKED]") {
		t.Errorf("got %q, expected [SSN_MASKED]", got)
	}
}
