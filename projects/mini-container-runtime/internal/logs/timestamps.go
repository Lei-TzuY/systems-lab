package logs

import (
	"fmt"
	"strings"
	"time"
)

// AddTimestampsToLogs prepends RFC3339 timestamps to log lines if missing.
func AddTimestampsToLogs(logContent string) string {
	lines := strings.Split(strings.TrimRight(logContent, "\n"), "\n")
	if len(lines) == 0 || (len(lines) == 1 && lines[0] == "") {
		return ""
	}

	nowStr := time.Now().Format(time.RFC3339)
	var sb strings.Builder

	for _, line := range lines {
		if strings.HasPrefix(line, "20") && len(line) > 20 && strings.Contains(line[:25], "T") {
			sb.WriteString(line + "\n")
		} else {
			sb.WriteString(fmt.Sprintf("%s %s\n", nowStr, line))
		}
	}

	return sb.String()
}
