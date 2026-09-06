package logs

import (
	"strings"
	"testing"
	"time"
)

func TestConvertLogsToUTC(t *testing.T) {
	nowStr := time.Now().Format(time.RFC3339)
	content := nowStr + " server started\n"

	res := ConvertLogsToUTC(content)
	if !strings.Contains(res, "server started") || !strings.Contains(res, "Z") {
		t.Fatalf("ConvertLogsToUTC = %s, want UTC timestamp with Z suffix", res)
	}
}
