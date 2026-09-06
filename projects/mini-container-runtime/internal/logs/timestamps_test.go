package logs

import (
	"strings"
	"testing"
)

func TestAddTimestampsToLogs(t *testing.T) {
	content := "hello world\nsecond line\n"
	result := AddTimestampsToLogs(content)
	if !strings.Contains(result, "hello world") || !strings.Contains(result, "T") {
		t.Fatalf("AddTimestampsToLogs = %s, want timestamped string", result)
	}
}
