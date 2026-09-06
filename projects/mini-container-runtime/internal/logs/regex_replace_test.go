package logs

import (
	"reflect"
	"testing"
)

func TestRegexReplacer_Replace(t *testing.T) {
	tests := []struct {
		name        string
		pattern     string
		replacement string
		input       string
		expected    string
		wantErr     bool
	}{
		{
			name:        "simple literal substitution",
			pattern:     "DEBUG",
			replacement: "INFO",
			input:       "2026-08-20 [DEBUG] Server initialized",
			expected:    "2026-08-20 [INFO] Server initialized",
			wantErr:     false,
		},
		{
			name:        "capture group backreference",
			pattern:     `token=([a-zA-Z0-9]+)`,
			replacement: `token=[REDACTED]`,
			input:       "request authorized token=secretXYZ123 status=200",
			expected:    "request authorized token=[REDACTED] status=200",
			wantErr:     false,
		},
		{
			name:        "capture group reordering",
			pattern:     `(\w+)=(\w+)`,
			replacement: `$2:$1`,
			input:       "foo=bar baz=qux",
			expected:    "bar:foo qux:baz",
			wantErr:     false,
		},
		{
			name:        "no match returns original",
			pattern:     "ERROR",
			replacement: "WARN",
			input:       "2026-08-20 [INFO] ok",
			expected:    "2026-08-20 [INFO] ok",
			wantErr:     false,
		},
		{
			name:        "empty pattern returns error",
			pattern:     "",
			replacement: "foo",
			input:       "bar",
			expected:    "",
			wantErr:     true,
		},
		{
			name:        "invalid regex returns error",
			pattern:     "[a-z(",
			replacement: "foo",
			input:       "bar",
			expected:    "",
			wantErr:     true,
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			replacer, err := NewRegexReplacer(tc.pattern, tc.replacement)
			if tc.wantErr {
				if err == nil {
					t.Fatalf("expected error for pattern %q, got nil", tc.pattern)
				}
				return
			}
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}
			got := replacer.Replace(tc.input)
			if got != tc.expected {
				t.Errorf("Replace() = %q, want %q", got, tc.expected)
			}
		})
	}
}

func TestRegexReplacer_ReplaceLines(t *testing.T) {
	replacer, err := NewRegexReplacer(`\b\d{4}-\d{2}-\d{2}\b`, "[DATE]")
	if err != nil {
		t.Fatalf("NewRegexReplacer failed: %v", err)
	}

	lines := []string{
		"2026-08-20 starting app",
		"no date in this line",
		"2026-08-21 finished app",
	}

	expected := []string{
		"[DATE] starting app",
		"no date in this line",
		"[DATE] finished app",
	}

	got := replacer.ReplaceLines(lines)
	if !reflect.DeepEqual(got, expected) {
		t.Errorf("ReplaceLines() = %v, want %v", got, expected)
	}
}
