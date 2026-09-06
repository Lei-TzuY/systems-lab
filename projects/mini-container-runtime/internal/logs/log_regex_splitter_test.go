package logs

import (
	"reflect"
	"testing"
)

func TestRegexLineSplitter_AssembleRecords(t *testing.T) {
	tests := []struct {
		name     string
		pattern  string
		joiner   string
		input    []string
		expected []string
	}{
		{
			name:    "groups stack traces by ISO date prefix",
			pattern: `^\d{4}-\d{2}-\d{2}`,
			joiner:  "\n",
			input: []string{
				"2026-08-20 10:00:00 [ERROR] NullPointerException",
				"    at com.example.App.main(App.java:42)",
				"    at com.example.Runner.run(Runner.java:10)",
				"2026-08-20 10:00:01 [INFO] Restarting service",
			},
			expected: []string{
				"2026-08-20 10:00:00 [ERROR] NullPointerException\n    at com.example.App.main(App.java:42)\n    at com.example.Runner.run(Runner.java:10)",
				"2026-08-20 10:00:01 [INFO] Restarting service",
			},
		},
		{
			name:    "empty input returns nil",
			pattern: `^INFO`,
			joiner:  "\n",
			input:   nil,
			expected: nil,
		},
		{
			name:    "custom joiner",
			pattern: `^\[`,
			joiner:  " --- ",
			input: []string{
				"[INFO] step 1",
				"detail a",
				"[INFO] step 2",
			},
			expected: []string{
				"[INFO] step 1 --- detail a",
				"[INFO] step 2",
			},
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			splitter, err := NewRegexLineSplitter(tc.pattern, tc.joiner)
			if err != nil {
				t.Fatalf("NewRegexLineSplitter failed: %v", err)
			}
			got := splitter.AssembleRecords(tc.input)
			if !reflect.DeepEqual(got, tc.expected) {
				t.Errorf("AssembleRecords() =\n%#v\nwant:\n%#v", got, tc.expected)
			}
		})
	}
}

func TestRegexLineSplitter_InvalidPattern(t *testing.T) {
	if _, err := NewRegexLineSplitter("", "\n"); err == nil {
		t.Error("expected error for empty pattern, got nil")
	}
	if _, err := NewRegexLineSplitter("[invalid(", "\n"); err == nil {
		t.Error("expected error for invalid regex, got nil")
	}
}
