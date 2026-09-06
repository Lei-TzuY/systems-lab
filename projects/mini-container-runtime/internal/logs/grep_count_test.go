package logs

import (
	"testing"
)

func TestCountGrepMatches(t *testing.T) {
	lines := []string{"ERROR: connection failed", "INFO: success", "ERROR: timeout ERROR"}
	count, err := CountGrepMatches(lines, "ERROR")
	if err != nil || count != 3 {
		t.Fatalf("CountGrepMatches error = %v, count = %d, want 3", err, count)
	}
}
