package logs

import (
	"strings"
	"testing"
)

func TestFormatLogToJSON(t *testing.T) {
	jsonStr, err := FormatLogToJSON("c123", "stdout", "server booting")
	if err != nil || !strings.Contains(jsonStr, `"containerId":"c123"`) || !strings.Contains(jsonStr, `"message":"server booting"`) {
		t.Fatalf("FormatLogToJSON error: %v (res=%s)", err, jsonStr)
	}
}
