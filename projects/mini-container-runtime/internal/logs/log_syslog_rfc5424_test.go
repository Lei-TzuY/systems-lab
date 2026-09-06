package logs

import (
	"strings"
	"testing"
	"time"
)

func TestInferSeverity(t *testing.T) {
	tests := []struct {
		input string
		want  int
	}{
		{"[FATAL] unexpected crash", 2},
		{"[ERROR] connection timeout", 3},
		{"[WARN] high memory usage", 4},
		{"[INFO] server listening on :8080", 6},
		{"[DEBUG] payload dump", 7},
	}

	for _, tc := range tests {
		t.Run(tc.input, func(t *testing.T) {
			got := InferSeverity(tc.input)
			if got != tc.want {
				t.Errorf("InferSeverity(%q) = %d, want %d", tc.input, got, tc.want)
			}
		})
	}
}

func TestSyslogRFC5424Formatter_FormatLine(t *testing.T) {
	formatter := NewSyslogRFC5424Formatter("node-01", "nginx-container", "1234")
	now := time.Date(2026, 8, 20, 12, 0, 0, 0, time.UTC)

	line := "2026-08-20T12:00:00Z [ERROR] Failed to bind port 80"
	got := formatter.FormatLine(line, now)

	// Expected format: <11>1 2026-08-20T12:00:00Z node-01 nginx-container 1234 - - ...
	if !strings.HasPrefix(got, "<11>1 2026-08-20T12:00:00Z node-01 nginx-container 1234 - -") {
		t.Errorf("unexpected syslog format: %q", got)
	}
}
