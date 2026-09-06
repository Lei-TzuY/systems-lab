package logs

import (
	"testing"
)

func TestTailLogLines(t *testing.T) {
	content := "line1\nline2\nline3\nline4\nline5\n"
	lines := TailLogLines(content, 2)
	if len(lines) != 2 || lines[0] != "line4" || lines[1] != "line5" {
		t.Fatalf("TailLogLines = %v, want [line4 line5]", lines)
	}
}
