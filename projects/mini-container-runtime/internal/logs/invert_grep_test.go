package logs

import (
	"testing"
)

func TestInvertGrepLogs(t *testing.T) {
	lines := []string{"DEBUG: detailed trace", "INFO: application ready", "DEBUG: processing item"}
	filtered, err := InvertGrepLogs(lines, "DEBUG")
	if err != nil || len(filtered) != 1 || filtered[0] != "INFO: application ready" {
		t.Fatalf("InvertGrepLogs error = %v, filtered = %v, want 1 INFO line", err, filtered)
	}
}
