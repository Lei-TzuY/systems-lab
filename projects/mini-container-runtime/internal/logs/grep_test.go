package logs

import (
	"testing"
)

func TestGrepLogLines(t *testing.T) {
	content := "INFO: system start\nERROR: database down\nINFO: user login\n"
	lines, err := GrepLogLines(content, "ERROR")
	if err != nil || len(lines) != 1 || lines[0] != "ERROR: database down" {
		t.Fatalf("GrepLogLines error: %v (lines=%v)", err, lines)
	}
}
