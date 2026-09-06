package logs

import (
	"strings"
	"testing"
)

func TestMaskSensitiveLogs(t *testing.T) {
	input := "Connecting with api_key=secret12345 to DB"
	masked := MaskSensitiveLogs(input)
	if strings.Contains(masked, "secret12345") || !strings.Contains(masked, "[REDACTED]") {
		t.Fatalf("MaskSensitiveLogs = %q, want [REDACTED]", masked)
	}
}
