package logs

import (
	"reflect"
	"testing"
)

func TestLogLevelFilter_DetectSeverity(t *testing.T) {
	tests := []struct {
		name     string
		line     string
		expected LogSeverity
	}{
		{
			name:     "bracket format info",
			line:     "2026-08-20 [INFO] Server started",
			expected: SeverityInfo,
		},
		{
			name:     "bracket format error",
			line:     "2026-08-20 [ERROR] Connection refused",
			expected: SeverityError,
		},
		{
			name:     "kv format warn",
			line:     `ts=123 level="warn" msg="disk space low"`,
			expected: SeverityWarn,
		},
		{
			name:     "kv format debug",
			line:     `ts=123 lvl=debug msg="query executed"`,
			expected: SeverityDebug,
		},
		{
			name:     "json format error",
			line:     `{"time":"2026-08-20","level":"error","msg":"fail"}`,
			expected: SeverityError,
		},
		{
			name:     "prefix format fatal",
			line:     "FATAL: out of memory",
			expected: SeverityFatal,
		},
		{
			name:     "unknown format",
			line:     "just some random stdout output",
			expected: SeverityUnknown,
		},
	}

	filter := NewLogLevelFilter("debug")
	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			got := filter.DetectSeverity(tc.line)
			if got != tc.expected {
				t.Errorf("DetectSeverity(%q) = %v, want %v", tc.line, got, tc.expected)
			}
		})
	}
}

func TestLogLevelFilter_FilterLines(t *testing.T) {
	lines := []string{
		"2026-08-20 [DEBUG] cache miss",
		"2026-08-20 [INFO] request handled",
		"2026-08-20 [WARN] slow query",
		"2026-08-20 [ERROR] db timeout",
		"2026-08-20 [FATAL] panic exit",
	}

	t.Run("filter min error", func(t *testing.T) {
		f := NewLogLevelFilter("error")
		got := f.FilterLines(lines)
		want := []string{
			"2026-08-20 [ERROR] db timeout",
			"2026-08-20 [FATAL] panic exit",
		}
		if !reflect.DeepEqual(got, want) {
			t.Errorf("got %v, want %v", got, want)
		}
	})

	t.Run("filter min warn", func(t *testing.T) {
		f := NewLogLevelFilter("warn")
		got := f.FilterLines(lines)
		want := []string{
			"2026-08-20 [WARN] slow query",
			"2026-08-20 [ERROR] db timeout",
			"2026-08-20 [FATAL] panic exit",
		}
		if !reflect.DeepEqual(got, want) {
			t.Errorf("got %v, want %v", got, want)
		}
	})
}
