package logs

import (
	"testing"
)

func TestFieldExtractor_Extract(t *testing.T) {
	tests := []struct {
		name     string
		fields   []string
		sep      string
		kvDelim  string
		line     string
		expected string
	}{
		{
			name:     "single field from space-separated",
			fields:   []string{"level"},
			line:     "ts=2025-01-01 level=info msg=hello",
			expected: "level=info",
		},
		{
			name:     "multiple fields from space-separated",
			fields:   []string{"level", "msg"},
			line:     "ts=2025-01-01 level=info msg=hello",
			expected: "level=info msg=hello",
		},
		{
			name:     "no matching fields returns empty",
			fields:   []string{"missing"},
			line:     "ts=2025-01-01 level=info msg=hello",
			expected: "",
		},
		{
			name:     "empty fields returns full line",
			fields:   []string{},
			line:     "ts=2025-01-01 level=info",
			expected: "ts=2025-01-01 level=info",
		},
		{
			name:     "custom separator comma",
			fields:   []string{"status"},
			sep:      ",",
			line:     "host=web1,status=200,latency=12ms",
			expected: "status=200",
		},
		{
			name:     "custom kv delimiter colon",
			fields:   []string{"port"},
			kvDelim:  ":",
			line:     "host:localhost port:8080 proto:tcp",
			expected: "port:8080",
		},
		{
			name:     "line with no kv pairs",
			fields:   []string{"level"},
			line:     "plain text log line without pairs",
			expected: "",
		},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			fe := NewFieldExtractor(tc.fields, tc.sep, tc.kvDelim)
			got := fe.Extract(tc.line)
			if got != tc.expected {
				t.Errorf("Extract() = %q, want %q", got, tc.expected)
			}
		})
	}
}

func TestFieldExtractor_ExtractLines(t *testing.T) {
	fe := NewFieldExtractor([]string{"level"}, "", "")
	lines := []string{
		"ts=1 level=info msg=hello",
		"no kv pairs here",
		"ts=2 level=warn msg=oops",
		"ts=3 msg=noLevel",
	}
	got := fe.ExtractLines(lines)
	if len(got) != 2 {
		t.Fatalf("ExtractLines() returned %d lines, want 2", len(got))
	}
	if got[0] != "level=info" {
		t.Errorf("ExtractLines()[0] = %q, want %q", got[0], "level=info")
	}
	if got[1] != "level=warn" {
		t.Errorf("ExtractLines()[1] = %q, want %q", got[1], "level=warn")
	}
}

func TestFieldExtractor_FormatExtracted_NoMatch(t *testing.T) {
	fe := NewFieldExtractor([]string{"missing"}, "", "")
	lines := []string{"key=val"}
	got := fe.FormatExtracted(lines)
	expected := "no matching fields [missing] found"
	if got != expected {
		t.Errorf("FormatExtracted() = %q, want %q", got, expected)
	}
}
